/// Une image complète (access unit) au format Annex B, start codes 00 00 00 01.
pub struct AccessUnit {
    pub data: Vec<u8>,
    pub is_keyframe: bool,
}

/// Découpe un flux Annex B en NAL units (sans les start codes).
pub fn split_nal_units(stream: &[u8]) -> Vec<&[u8]> {
    // Position du premier octet de chaque NAL (juste après un start code 00 00 01).
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 3 <= stream.len() {
        if stream[i] == 0 && stream[i + 1] == 0 && stream[i + 2] == 1 {
            starts.push(i + 3);
            i += 3;
        } else {
            i += 1;
        }
    }

    let mut nals = Vec::with_capacity(starts.len());
    for (k, &s) in starts.iter().enumerate() {
        let mut end = if k + 1 < starts.len() {
            starts[k + 1] - 3
        } else {
            stream.len()
        };
        // Retire les zéros de fin : ils appartiennent au start code à 4 octets suivant.
        while end > s && stream[end - 1] == 0 {
            end -= 1;
        }
        if end > s {
            nals.push(&stream[s..end]);
        }
    }
    nals
}

/// Regroupe les NAL units en images complètes.
pub fn group_access_units(nals: &[&[u8]]) -> Vec<AccessUnit> {
    let mut units = Vec::new();
    let mut current: Vec<&[u8]> = Vec::new();
    let mut has_vcl = false;
    let mut keyframe = false;

    for &nal in nals {
        let nal_type = nal[0] & 0x1F;
        let is_vcl = nal_type == 1 || nal_type == 5; // tranche d'image (5 = IDR, keyframe)
        let is_header = (6..=9).contains(&nal_type); // SEI, SPS, PPS, AUD
        // Premier bit du header de tranche à 1 => first_mb_in_slice == 0 => nouvelle image.
        let first_slice = is_vcl && nal.len() > 1 && nal[1] & 0x80 != 0;

        if has_vcl && (is_header || first_slice) {
            units.push(build_unit(&current, keyframe));
            current.clear();
            has_vcl = false;
            keyframe = false;
        }

        current.push(nal);
        if is_vcl {
            has_vcl = true;
            keyframe |= nal_type == 5;
        }
    }
    if has_vcl {
        units.push(build_unit(&current, keyframe));
    }
    units
}

fn build_unit(nals: &[&[u8]], is_keyframe: bool) -> AccessUnit {
    let mut data = Vec::new();
    for nal in nals {
        data.extend_from_slice(&[0, 0, 0, 1]);
        data.extend_from_slice(nal);
    }
    AccessUnit { data, is_keyframe }
}

/// Cherche le SPS (type 7) et le PPS (type 8) dans une image au format Annex B.
/// Renvoie les NAL sans start code, octet d'en-tête de NAL compris.
pub fn find_parameter_sets(annexb: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    let mut sps = None;
    let mut pps = None;
    for nal in split_nal_units(annexb) {
        match nal[0] & 0x1F {
            7 if sps.is_none() => sps = Some(nal.to_vec()),
            8 if pps.is_none() => pps = Some(nal.to_vec()),
            _ => {}
        }
    }
    Some((sps?, pps?))
}

/// Convertit une image Annex B au format AVCC : chaque NAL est précédé de sa
/// longueur sur 4 octets. SPS, PPS et AUD sont retirés, car ils sont décrits
/// une fois pour toutes dans l'en-tête du fichier MP4.
pub fn annexb_to_avcc(annexb: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(annexb.len());
    for nal in split_nal_units(annexb) {
        if matches!(nal[0] & 0x1F, 7 | 8 | 9) {
            continue;
        }
        out.extend_from_slice(&(nal.len() as u32).to_be_bytes());
        out.extend_from_slice(nal);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_and_groups() {
        let stream = [
            0, 0, 0, 1, 0x67, 0xAA, // SPS
            0, 0, 0, 1, 0x68, 0xBB, // PPS
            0, 0, 1, 0x65, 0x88, 0xCC, // IDR (keyframe)
            0, 0, 0, 1, 0x41, 0x9A, 0xDD, // image P
        ];
        let nals = split_nal_units(&stream);
        assert_eq!(nals.len(), 4);
        assert_eq!(nals[2], &[0x65, 0x88, 0xCC][..]);

        let units = group_access_units(&nals);
        assert_eq!(units.len(), 2);
        assert!(units[0].is_keyframe);
        assert!(!units[1].is_keyframe);
    }
        #[test]
    fn finds_parameter_sets() {
        let au = [
            0, 0, 0, 1, 0x67, 0xAA, //
            0, 0, 0, 1, 0x68, 0xBB, //
            0, 0, 0, 1, 0x65, 0x88, 0xCC,
        ];
        let (sps, pps) = find_parameter_sets(&au).unwrap();
        assert_eq!(sps, vec![0x67, 0xAA]);
        assert_eq!(pps, vec![0x68, 0xBB]);
    }

    #[test]
    fn converts_to_avcc() {
        let au = [
            0, 0, 0, 1, 0x67, 0xAA, //
            0, 0, 0, 1, 0x68, 0xBB, //
            0, 0, 0, 1, 0x65, 0x88, 0xCC,
        ];
        assert_eq!(annexb_to_avcc(&au), vec![0, 0, 0, 3, 0x65, 0x88, 0xCC]);
    }
}