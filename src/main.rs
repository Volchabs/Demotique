#![allow(dead_code)]

mod buffer;
mod packet;
mod source;
mod h264;
mod mux;

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use buffer::ReplayBuffer;
use source::FileSource;


const CLIP_DURATION: Duration = Duration::from_secs(30);

fn main() {
    let buffer = Arc::new(Mutex::new(ReplayBuffer::new(CLIP_DURATION)));
    let _source = FileSource::start("assets/test.h264", buffer.clone()).expect("impossible de lire assets/test.h264");

    // 35 s de démo : le tampon se remplit, puis se stabilise
    for i in 1..=35 {
        thread::sleep(Duration::from_secs(1));
        if i % 5 == 0 {
            let b = buffer.lock().unwrap();
            println!(
                "tampon : {:.1} s, {} paquets, {} Ko",
                b.buffered_duration().as_secs_f32(),
                b.len(),
                b.total_bytes() / 1024
            );
        }
    }

    // Simule l'appui sur le raccourci : le verrou n'est tenu que le temps de la copie.
    let clip = { buffer.lock().unwrap().snapshot() };

    match clip {
    Some(packets) => {
        // (tes println! existants)
        mux::write_mp4(std::path::Path::new("clip.mp4"), &packets, 1280, 720).expect("écriture du MP4");
        println!("clip.mp4 écrit");
    }   
    None => println!("pas encore de keyframe"),
}
}