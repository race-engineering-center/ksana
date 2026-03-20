use std::fs::File;
use std::io::BufReader;

use humantime::format_duration;

use crate::{io::Loader, traits::PlayError};

pub fn run(input_file: &str) -> Result<(), PlayError> {
    let file = match File::open(input_file) {
        Ok(f) => f,
        Err(e) => {
            return Err(PlayError::FailedToOpenFile(e));
        }
    };

    let reader = BufReader::new(file);
    let mut loader = match Loader::new(reader) {
        Ok(l) => l,
        Err(e) => {
            return Err(PlayError::FailedToReadHeader(e));
        }
    };

    let fps = loader.fps();
    let id = loader.id();

    println!(
        "Ksana recording: {} (sim: {}, fps: {})",
        input_file,
        std::str::from_utf8(&id).unwrap_or("????"),
        fps
    );

    let mut exited_cleanly = false;
    let mut frame_counter: u64 = 0;
    loop {
        let _ = match loader.seek() {
            Ok(Some(data)) => data,
            Ok(None) => {
                exited_cleanly = true;
                break;
            }
            Err(e) => {
                eprintln!("Error reading frame {}: {}", frame_counter, e);
                break;
            }
        };

        frame_counter += 1;
    }

    if exited_cleanly {
        println!("Total frames: {}", frame_counter);
    } else {
        println!("Stopped prematurely. Total frames: {}", frame_counter);
    }
    println!(
        "Total duration: {}",
        format_duration(std::time::Duration::from_secs(
            (frame_counter as f64 / fps as f64) as u64
        ))
    );

    Ok(())
}
