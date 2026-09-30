use std::error::Error;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::Duration;

use mp4::{
    AvcConfig, Bytes, FourCC, MediaConfig, Mp4Config, Mp4Sample, Mp4Writer, TrackConfig, TrackType,
};

use crate::h264;
use crate::packet::{EncodedPacket, StreamKind};

/// 90 000 ticks/s : le standard en vidéo, et 30 images/s tombent juste (3000 ticks par image).
const TIMESCALE: u32 = 90_000;

fn fourcc(s: &str) -> FourCC {
    s.parse().unwrap()
}

/// Écrit la partie vidéo d'un clip dans un fichier MP4.
/// La résolution est fournie par l'appelant (la capture la connaît).
pub fn write_mp4(
    path: &Path,
    packets: &[EncodedPacket],
    width: u16,
    height: u16,
) -> Result<(), Box<dyn Error>> {
    let video: Vec<&EncodedPacket> = packets
        .iter()
        .filter(|p| p.stream == StreamKind::Video)
        .collect();

    let first = video.first().ok_or("clip vide")?;
    if !first.is_keyframe {
        return Err("le clip ne commence pas par une keyframe".into());
    }
    let (sps, pps) = h264::find_parameter_sets(&first.data)
        .ok_or("SPS/PPS introuvables dans la première image")?;

    let track = TrackConfig {
        track_type: TrackType::Video,
        timescale: TIMESCALE,
        language: "und".to_string(),
        media_conf: MediaConfig::AvcConfig(AvcConfig {
            width,
            height,
            seq_param_set: sps,
            pic_param_set: pps,
        }),
    };
    let config = Mp4Config {
        major_brand: fourcc("isom"),
        minor_version: 512,
        compatible_brands: vec![fourcc("isom"), fourcc("iso2"), fourcc("avc1"), fourcc("mp41")],
        timescale: 1000,
    };

    // Les durées des images se déduisent des écarts entre timestamps.
    let to_ticks = |d: Duration| (d.as_micros() * TIMESCALE as u128 / 1_000_000) as u64;
    let origin = to_ticks(first.pts);
    let starts: Vec<u64> = video
        .iter()
        .map(|p| to_ticks(p.pts).saturating_sub(origin))
        .collect();

    let mut writer = Mp4Writer::write_start(BufWriter::new(File::create(path)?), &config)?;
    writer.add_track(&track)?;

    for (i, packet) in video.iter().enumerate() {
        let duration = if i + 1 < starts.len() {
            starts[i + 1] - starts[i]
        } else if i > 0 {
            starts[i] - starts[i - 1] // dernière image : on reprend la durée précédente
        } else {
            TIMESCALE as u64 / 30
        };

        writer.write_sample(
            1, // identifiant de la première piste ajoutée
            &Mp4Sample {
                start_time: starts[i],
                duration: duration as u32,
                rendering_offset: 0, // pas de B-frames
                is_sync: packet.is_keyframe,
                bytes: Bytes::from(h264::annexb_to_avcc(&packet.data)),
            },
        )?;
    }

    writer.write_end()?;
    writer.into_writer().flush()?;
    Ok(())
}