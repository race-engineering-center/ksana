use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::io::{IOError, Loader};
use crate::sims::assettocorsa::player::AssettoCorsaPlayer;
use crate::sims::iracing::player::IRacingPlayer;
use crate::sleeper::AdaptiveSleeper;
use crate::{Player, Sleeper};

#[derive(thiserror::Error, Debug)]
pub enum PlayError {
    #[error("Failed to open file: {0}")]
    FailedToOpenFile(std::io::Error),

    #[error("Failed to read header: {0}")]
    FailedToReadHeader(IOError),

    #[error("Unknown simulator ID: {0}")]
    UnknownSimError(String),

    #[error("Failed to initialize player: {0}")]
    FailedToInitializePlayer(anyhow::Error),

    #[error("Failed to load frame: {0}")]
    FailedToLoadFrame(IOError),

    #[error("Failed to update player: {0}")]
    FailedToUpdatePlayer(anyhow::Error),
}

pub enum PlayResult {
    EndOfFile,
    QuitRequested,
}

pub fn run(quit_flag: Arc<AtomicBool>, input_file: &str) -> Result<PlayResult, PlayError> {
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
        "Playing: {} (sim: {}, fps: {})",
        input_file,
        std::str::from_utf8(&id).unwrap_or("????"),
        fps
    );

    let mut player: Box<dyn Player> = match &id {
        b"irac" => Box::new(IRacingPlayer::new()),
        b"acsa" => Box::new(AssettoCorsaPlayer::new()),
        _ => {
            return Err(PlayError::UnknownSimError(
                std::str::from_utf8(&id).unwrap_or("????").to_string(),
            ));
        }
    };

    if let Err(e) = player.initialize() {
        return Err(PlayError::FailedToInitializePlayer(e));
    }

    println!("Player initialized, starting playback");

    let sleeper = AdaptiveSleeper::default();
    let tick_ms = 1000.0 / fps as f64;

    let mut result = PlayResult::QuitRequested;

    while !quit_flag.load(Ordering::Relaxed) {
        let start = std::time::Instant::now();

        let frame = match loader.load() {
            Ok(Some(data)) => data,
            Ok(None) => {
                result = PlayResult::EndOfFile;
                break;
            }
            Err(e) => {
                return Err(PlayError::FailedToLoadFrame(e));
            }
        };

        if let Err(e) = player.update(&frame) {
            return Err(PlayError::FailedToUpdatePlayer(e));
        }

        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        if elapsed_ms < tick_ms {
            sleeper.sleep_ms((tick_ms - elapsed_ms) as u64);
        }
    }

    player.stop();

    println!("Player stopped.");
    println!("You can now close this window.");

    Ok(result)
}
