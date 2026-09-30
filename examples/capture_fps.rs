#[cfg(windows)]
mod imp {
    use std::time::{Duration, Instant};

    use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
    use windows_capture::frame::Frame;
    use windows_capture::graphics_capture_api::InternalCaptureControl;
    use windows_capture::graphics_capture_picker::GraphicsCapturePicker;
    use windows_capture::settings::{
        ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
        MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
    };

    struct FpsCounter {
        start: Instant,
        last_report: Instant,
        frames: u32,
    }

    impl GraphicsCaptureApiHandler for FpsCounter {
        type Flags = ();
        type Error = Box<dyn std::error::Error + Send + Sync>;

        fn new(_ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
            let now = Instant::now();
            Ok(Self { start: now, last_report: now, frames: 0 })
        }

        fn on_frame_arrived(
            &mut self,
            frame: &mut Frame,
            capture_control: InternalCaptureControl,
        ) -> Result<(), Self::Error> {
            self.frames += 1;
            if self.last_report.elapsed() >= Duration::from_secs(1) {
                println!("{} images/s ({}x{})", self.frames, frame.width(), frame.height());
                self.frames = 0;
                self.last_report = Instant::now();
            }
            if self.start.elapsed() >= Duration::from_secs(10) {
                capture_control.stop();
            }
            Ok(())
        }

        fn on_closed(&mut self) -> Result<(), Self::Error> {
            println!("capture terminée");
            Ok(())
        }
    }

    pub fn run() {
        // Ouvre le sélecteur Windows : choisis un écran ou une fenêtre (un jeu, par exemple).
        let item = GraphicsCapturePicker::pick_item().expect("échec du sélecteur");
        let Some(item) = item else {
            println!("rien sélectionné");
            return;
        };

        let settings = Settings::new(
            item,
            CursorCaptureSettings::Default,
            DrawBorderSettings::Default,
            SecondaryWindowSettings::Default,
            MinimumUpdateIntervalSettings::Default,
            DirtyRegionSettings::Default,
            ColorFormat::Rgba8,
            (), // pas de paramètres supplémentaires
        );

        FpsCounter::start(settings).expect("capture impossible");
    }
}

fn main() {
    #[cfg(windows)]
    imp::run();
    #[cfg(not(windows))]
    println!("Cet exemple ne fonctionne que sous Windows.");
}