use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::buffer::ReplayBuffer;
use crate::packet::{EncodedPacket, StreamKind};
use crate::h264::{self, AccessUnit};

const VIDEO_INTERVAL_US: u64 = 33_333; // 30 images/s
const AUDIO_INTERVAL_US: u64 = 21_333; // paquet AAC de 1024 échantillons à 48 kHz
const GOP_SIZE: u64 = 30; // une keyframe par seconde

/// Produit de faux paquets en temps réel dans un thread dédié.
pub struct FakeSource {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl FakeSource {
    pub fn start(buffer: Arc<Mutex<ReplayBuffer>>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = stop.clone();
        let handle = thread::spawn(move || run(buffer, stop_flag));
        Self { stop, handle: Some(handle) }
    }
}

impl Drop for FakeSource {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn run(buffer: Arc<Mutex<ReplayBuffer>>, stop: Arc<AtomicBool>) {
    let start = Instant::now();
    let mut video_n: u64 = 0;
    let mut audio_n: u64 = 0;

    while !stop.load(Ordering::Relaxed) {
        let now_us = start.elapsed().as_micros() as u64;
        let mut batch: Vec<EncodedPacket> = Vec::new();

        while video_n * VIDEO_INTERVAL_US <= now_us {
            let key = video_n % GOP_SIZE == 0;
            let size = if key { 50_000 } else { 8_000 };
            let pts = Duration::from_micros(video_n * VIDEO_INTERVAL_US);
            batch.push(EncodedPacket::new(StreamKind::Video, vec![0; size], pts, key));
            video_n += 1;
        }
        while audio_n * AUDIO_INTERVAL_US <= now_us {
            let pts = Duration::from_micros(audio_n * AUDIO_INTERVAL_US);
            batch.push(EncodedPacket::new(StreamKind::Audio, vec![0; 400], pts, true));
            audio_n += 1;
        }

        // Audio et vidéo entrelacés dans l'ordre des timestamps,
        // comme un vrai muxeur les attendra.
        batch.sort_by_key(|p| p.pts);

        if !batch.is_empty() {
            let mut buf = buffer.lock().unwrap();
            for p in batch {
                buf.push(p);
            }
        }

        thread::sleep(Duration::from_millis(5));
    }
}

/// Lit un fichier H.264 Annex B et le rejoue en temps réel à 30 images/s, en boucle.
pub struct FileSource {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl FileSource {
    pub fn start(path: &str, buffer: Arc<Mutex<ReplayBuffer>>) -> std::io::Result<Self> {
        let stream = std::fs::read(path)?;
        let nals = h264::split_nal_units(&stream);
        let units = h264::group_access_units(&nals);
        if units.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "aucune image H.264 trouvée dans le fichier",
            ));
        }

        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = stop.clone();
        let handle = thread::spawn(move || run_file(units, buffer, stop_flag));
        Ok(Self { stop, handle: Some(handle) })
    }
}

impl Drop for FileSource {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn run_file(units: Vec<AccessUnit>, buffer: Arc<Mutex<ReplayBuffer>>, stop: Arc<AtomicBool>) {
    let start = Instant::now();
    // `cycle()` relance le fichier quand il est fini ; le pts continue de croître.
    for (n, unit) in units.iter().cycle().enumerate() {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let pts_us = n as u64 * VIDEO_INTERVAL_US;
        let target = start + Duration::from_micros(pts_us);
        let now = Instant::now();
        if target > now {
            thread::sleep(target - now);
        }
        let packet = EncodedPacket::new(
            StreamKind::Video,
            unit.data.clone(),
            Duration::from_micros(pts_us),
            unit.is_keyframe,
        );
        buffer.lock().unwrap().push(packet);
    }
}