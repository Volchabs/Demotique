use std::collections::VecDeque;
use std::time::Duration;

use crate::packet::{EncodedPacket, StreamKind};

/// Tampon circulaire de paquets encodés.
///
/// Il garde au moins `max_duration` de contenu, en commençant toujours
/// sur une keyframe vidéo : il contient donc entre `max_duration` et
/// `max_duration + 1 GOP` (un GOP = intervalle entre deux keyframes).
pub struct ReplayBuffer {
    max_duration: Duration,
    packets: VecDeque<EncodedPacket>,
    /// Numéro de séquence absolu du paquet en tête de `packets`.
    base_seq: u64,
    /// (numéro de séquence absolu, pts) de chaque keyframe vidéo présente.
    keyframes: VecDeque<(u64, Duration)>,
    newest_pts: Duration,
    total_bytes: usize,
}

impl ReplayBuffer {
    pub fn new(max_duration: Duration) -> Self {
        Self {
            max_duration,
            packets: VecDeque::new(),
            base_seq: 0,
            keyframes: VecDeque::new(),
            newest_pts: Duration::ZERO,
            total_bytes: 0,
        }
    }

    pub fn push(&mut self, packet: EncodedPacket) {
        let seq = self.base_seq + self.packets.len() as u64;
        if packet.stream == StreamKind::Video && packet.is_keyframe {
            self.keyframes.push_back((seq, packet.pts));
        }
        self.newest_pts = self.newest_pts.max(packet.pts);
        self.total_bytes += packet.data.len();
        self.packets.push_back(packet);
        self.evict();
    }

    /// Supprime le début du tampon tant que la keyframe suivante
    /// laisse encore au moins `max_duration` de contenu.
    fn evict(&mut self) {
        let limit = self.newest_pts.saturating_sub(self.max_duration);
        while self.keyframes.len() >= 2 && self.keyframes[1].1 <= limit {
            let next_seq = self.keyframes[1].0;
            let to_drop = (next_seq - self.base_seq) as usize;
            for _ in 0..to_drop {
                if let Some(p) = self.packets.pop_front() {
                    self.total_bytes -= p.data.len();
                }
            }
            self.base_seq = next_seq;
            self.keyframes.pop_front();
        }
    }

    /// Copie le contenu à sauvegarder : commence à la première keyframe,
    /// et écarte l'audio antérieur à celle-ci pour garder l'image et le son alignés.
    /// Renvoie `None` tant qu'aucune keyframe n'est arrivée.
    pub fn snapshot(&self) -> Option<Vec<EncodedPacket>> {
        let &(kf_seq, kf_pts) = self.keyframes.front()?;
        let start = (kf_seq - self.base_seq) as usize;

        let clip = self
            .packets
            .iter()
            .enumerate()
            .filter(|(i, p)| match p.stream {
                StreamKind::Video => *i >= start,
                StreamKind::Audio => p.pts >= kf_pts,
            })
            .map(|(_, p)| p.clone())
            .collect();
        Some(clip)
    }

    pub fn len(&self) -> usize {
        self.packets.len()
    }

    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// Durée réellement exploitable pour un clip.
    pub fn buffered_duration(&self) -> Duration {
        match self.keyframes.front() {
            Some(&(_, pts)) => self.newest_pts.saturating_sub(pts),
            None => Duration::ZERO,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn video(ms: u64, key: bool) -> EncodedPacket {
        EncodedPacket::new(StreamKind::Video, vec![0; 1000], Duration::from_millis(ms), key)
    }

    fn audio(ms: u64) -> EncodedPacket {
        EncodedPacket::new(StreamKind::Audio, vec![0; 100], Duration::from_millis(ms), true)
    }

    #[test]
    fn empty_buffer_has_no_snapshot() {
        let buf = ReplayBuffer::new(Duration::from_secs(5));
        assert!(buf.snapshot().is_none());
    }

    #[test]
    fn no_snapshot_without_keyframe() {
        let mut buf = ReplayBuffer::new(Duration::from_secs(5));
        buf.push(video(0, false));
        buf.push(video(100, false));
        assert!(buf.snapshot().is_none());
    }

    #[test]
    fn snapshot_starts_with_keyframe() {
        let mut buf = ReplayBuffer::new(Duration::from_secs(5));
        buf.push(video(0, true));
        buf.push(video(100, false));
        let clip = buf.snapshot().unwrap();
        assert_eq!(clip.len(), 2);
        assert!(clip[0].is_keyframe);
    }

    #[test]
    fn old_audio_before_first_keyframe_is_dropped() {
        let mut buf = ReplayBuffer::new(Duration::from_secs(5));
        buf.push(audio(0));
        buf.push(audio(50));
        buf.push(video(100, true));
        buf.push(audio(120));
        let clip = buf.snapshot().unwrap();
        assert_eq!(clip.len(), 2);
        assert_eq!(clip[0].stream, StreamKind::Video);
    }

    #[test]
    fn evicts_old_data_but_keeps_max_duration() {
        // 10 images/s, une keyframe par seconde, 20 s de capture, tampon de 5 s
        let mut buf = ReplayBuffer::new(Duration::from_secs(5));
        for i in 0..200u64 {
            buf.push(video(i * 100, i % 10 == 0));
        }

        let d = buf.buffered_duration();
        assert!(d >= Duration::from_secs(5) && d < Duration::from_secs(6));
        assert_eq!(buf.len(), 60); // de 14,0 s à 19,9 s
        assert_eq!(buf.total_bytes(), 60 * 1000);

        let clip = buf.snapshot().unwrap();
        assert!(clip[0].is_keyframe);
        assert_eq!(clip[0].pts, Duration::from_secs(14));
    }
}