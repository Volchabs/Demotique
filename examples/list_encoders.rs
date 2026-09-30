#[cfg(windows)]
mod imp {
    use windows::core::{GUID, PWSTR};
    use windows::Win32::Media::MediaFoundation::{
        IMFActivate, IMFAttributes, MFMediaType_Video, MFShutdown, MFStartup, MFTEnumEx,
        MFVideoFormat_H264, MFT_CATEGORY_VIDEO_ENCODER, MFT_ENUM_FLAG, MFT_ENUM_FLAG_HARDWARE,
        MFT_ENUM_FLAG_SORTANDFILTER, MFT_ENUM_FLAG_SYNCMFT, MFT_ENUM_HARDWARE_VENDOR_ID_Attribute,
        MFT_FRIENDLY_NAME_Attribute, MFT_REGISTER_TYPE_INFO, MF_VERSION,
    };
    use windows::Win32::System::Com::{CoInitializeEx, CoTaskMemFree, COINIT_MULTITHREADED};

    /// Lit un attribut texte d'un objet Media Foundation.
    unsafe fn get_string(attrs: &IMFAttributes, key: &GUID) -> Option<String> {
        let mut ptr = PWSTR::null();
        let mut len = 0u32;
        attrs.GetAllocatedString(key, &mut ptr, &mut len).ok()?;
        let text = ptr.to_string().ok();
        CoTaskMemFree(Some(ptr.0 as *const _));
        text
    }

    fn vendor_name(id: &str) -> &'static str {
        let id = id.to_uppercase();
        if id.contains("10DE") {
            "NVIDIA"
        } else if id.contains("1002") || id.contains("1022") {
            "AMD"
        } else if id.contains("8086") {
            "Intel"
        } else {
            "inconnu"
        }
    }

    unsafe fn list_h264_encoders(label: &str, flags: MFT_ENUM_FLAG) -> windows::core::Result<()> {
        // Pour un encodeur, H.264 est le format de *sortie*.
        let output = MFT_REGISTER_TYPE_INFO {
            guidMajorType: MFMediaType_Video,
            guidSubtype: MFVideoFormat_H264,
        };

        let mut activates: *mut Option<IMFActivate> = std::ptr::null_mut();
        let mut count = 0u32;
        MFTEnumEx(
            MFT_CATEGORY_VIDEO_ENCODER,
            flags,
            None,
            Some(&output),
            &mut activates,
            &mut count,
        )?;

        println!("{label} : {count} trouvé(s)");
        if activates.is_null() {
            return Ok(());
        }

        // `take()` sort chaque objet du tableau : il est libéré proprement en fin d'itération.
        let slots = std::slice::from_raw_parts_mut(activates, count as usize);
        for slot in slots.iter_mut() {
            if let Some(activate) = slot.take() {
                let name = get_string(&activate, &MFT_FRIENDLY_NAME_Attribute)
                    .unwrap_or_else(|| "(sans nom)".to_string());
                let vendor = get_string(&activate, &MFT_ENUM_HARDWARE_VENDOR_ID_Attribute)
                    .map(|v| format!(" [fabricant : {} / {v}]", vendor_name(&v)))
                    .unwrap_or_default();
                println!("  - {name}{vendor}");
            }
        }
        // Le tableau lui-même est alloué par Windows.
        CoTaskMemFree(Some(activates as *const _));
        Ok(())
    }

    pub fn run() -> windows::core::Result<()> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
            MFStartup(MF_VERSION, 0)?; // 0 = MFSTARTUP_FULL

            list_h264_encoders(
                "Encodeurs matériels (GPU)",
                MFT_ENUM_FLAG_HARDWARE | MFT_ENUM_FLAG_SORTANDFILTER,
            )?;
            list_h264_encoders(
                "Encodeurs logiciels (CPU)",
                MFT_ENUM_FLAG_SYNCMFT | MFT_ENUM_FLAG_SORTANDFILTER,
            )?;

            MFShutdown()?;
        }
        Ok(())
    }
}

fn main() {
    #[cfg(windows)]
    {
        if let Err(e) = imp::run() {
            eprintln!("erreur : {e}");
        }
    }
    #[cfg(not(windows))]
    println!("Cet exemple ne fonctionne que sous Windows.");
}