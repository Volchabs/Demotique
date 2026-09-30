#![allow(dead_code)]

mod buffer;
mod h264;
mod mux;
mod packet;
mod recorder;
mod source;

use std::io::BufRead;
use std::path::PathBuf;
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use recorder::Recorder;
use source::FileSource;

const CLIP_DURATION: Duration = Duration::from_secs(30);

enum Event {
    SaveClip,
    Quit,
}

fn main() {
    let recorder = Arc::new(Recorder::new(
        CLIP_DURATION,
        PathBuf::from("clips"),
        1280,
        720,
    ));
    let _source = FileSource::start("assets/test.h264", recorder.buffer())
        .expect("impossible de lire assets/test.h264");

    let (tx, rx) = mpsc::channel();

    // Remplace le raccourci clavier global : Entrée = clip, q = quitter.
    thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            let event = if line.trim() == "q" {
                Event::Quit
            } else {
                Event::SaveClip
            };
            if tx.send(event).is_err() {
                break;
            }
        }
    });

    println!("Démotique prêt : Entrée = sauvegarder un clip, q = quitter");

    while let Ok(event) = rx.recv() {
        match event {
            Event::SaveClip => {
                let recorder = recorder.clone();
                // Thread dédié : la boucle d'événements reste réactive pendant le muxage.
                thread::spawn(move || {
                    let start = Instant::now();
                    match recorder.save_clip() {
                        Ok(path) => println!(
                            "clip sauvegardé : {} ({:.0?})",
                            path.display(),
                            start.elapsed()
                        ),
                        Err(e) => eprintln!("échec de la sauvegarde : {e}"),
                    }
                });
            }
            Event::Quit => break,
        }
    }
}