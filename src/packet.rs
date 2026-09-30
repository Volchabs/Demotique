use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamKind {
    Video,
    Audio,
}

/// Un paquet déjà encodé (H.264, AAC...), prêt à être écrit dans un conteneur.
#[derive(Debug, Clone)]
pub struct EncodedPacket {
    pub stream: StreamKind,
    /// `Arc<[u8]>` : cloner un paquet ne copie pas les données,
    /// ce qui rend la sauvegarde d'un clip quasi gratuite.
    pub data: Arc<[u8]>,
    /// Horodatage depuis le début de la capture.
    pub pts: Duration,
    /// Pour la vidéo : image clé (point de départ possible d'un clip).
    pub is_keyframe: bool,
}

impl EncodedPacket {
    pub fn new(stream: StreamKind, data: Vec<u8>, pts: Duration, is_keyframe: bool) -> Self {
        Self {
            stream,
            data: Arc::from(data),
            pts,
            is_keyframe,
        }
    }
}