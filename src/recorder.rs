use std::error::Error;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::buffer::ReplayBuffer;
use crate::mux;

/// Regroupe le tampon et tout ce qu'il faut pour sauvegarder un clip.
pub struct Recorder {
    buffer: Arc<Mutex<ReplayBuffer>>,
    output_dir: PathBuf,
    width: u16,
    height: u16,
}

impl Recorder {
    pub fn new(clip_duration: Duration, output_dir: PathBuf, width: u16, height: u16) -> Self {
        Self {
            buffer: Arc::new(Mutex::new(ReplayBuffer::new(clip_duration))),
            output_dir,
            width,
            height,
        }
    }

    /// Le tampon partagé, à donner à la source de capture.
    pub fn buffer(&self) -> Arc<Mutex<ReplayBuffer>> {
        self.buffer.clone()
    }

    /// Sauvegarde les dernières secondes dans un nouveau fichier MP4.
    pub fn save_clip(&self) -> Result<PathBuf, Box<dyn Error>> {
        // Le verrou n'est tenu que le temps de la copie : le muxage se fait ensuite sans lui.
        let packets = self
            .buffer
            .lock()
            .unwrap()
            .snapshot()
            .ok_or("pas encore de keyframe dans le tampon")?;

        std::fs::create_dir_all(&self.output_dir)?;
        let name = format!("clip_{}.mp4", chrono::Local::now().format("%Y-%m-%d_%H-%M-%S"));
        let path = self.output_dir.join(name);
        mux::write_mp4(&path, &packets, self.width, self.height)?;
        Ok(path)
    }
}