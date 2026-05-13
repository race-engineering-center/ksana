use std::fs::File;
use std::io::BufWriter;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::io::{IOError, Saver};
use crate::sims::assettocorsa::connector::AssettoCorsaConnector;
use crate::sims::iracing::connector::IRacingConnector;
use crate::sleeper::AdaptiveSleeper;
use crate::{Connector, Sleeper};

struct ConnectorGuard<'a> {
    inner: &'a mut dyn Connector,
}

impl<'a> ConnectorGuard<'a> {
    pub fn new(connector: &'a mut dyn Connector) -> Self {
        ConnectorGuard { inner: connector }
    }
}

impl<'a> Drop for ConnectorGuard<'a> {
    fn drop(&mut self) {
        self.inner.disconnect();
    }
}

impl<'a> Deref for ConnectorGuard<'a> {
    type Target = dyn Connector + 'a;
    fn deref(&self) -> &Self::Target {
        self.inner
    }
}

impl<'a> DerefMut for ConnectorGuard<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner
    }
}

#[derive(thiserror::Error, Debug)]
pub enum RecordingError {
    #[error("Failed to save frame: {0}")]
    SavingFrameFailed(#[from] IOError),
}

pub enum RecordingFinished {
    SimDisconnected,
    QuitRequested,
    MaxDurationReached,
}

#[derive(thiserror::Error, Debug)]
pub enum RecordError {
    #[error("Failed to create file: {0}")]
    CreateFileError(std::io::Error),

    #[error("Failed to initialize saver: {0}")]
    SaverInitError(IOError),

    #[error("Flush failed: {0}")]
    FlushFailed(IOError),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Recording(#[from] RecordingError),

    #[error(transparent)]
    Record(#[from] RecordError),

    #[error("Invalid simulator ID")]
    InvalidSimId,

    #[error("Failed to parse max duration")]
    ParseMaxDuration(#[from] ParseDurationError),
}

#[derive(thiserror::Error, Debug)]
pub enum ParseDurationError {
    #[error("Invalid format")]
    InvalidFormat,
}

fn parse_duration(arg: &str) -> Result<Duration, ParseDurationError> {
    if arg.is_empty() {
        return Err(ParseDurationError::InvalidFormat);
    }

    if let Some(stripped) = arg.strip_suffix('s') {
        let seconds: u64 = stripped
            .parse()
            .map_err(|_| ParseDurationError::InvalidFormat)?;
        return Ok(Duration::from_secs(seconds));
    }

    if let Some(stripped) = arg.strip_suffix('m') {
        let minutes: u64 = stripped
            .parse()
            .map_err(|_| ParseDurationError::InvalidFormat)?;
        return Ok(Duration::from_secs(minutes * 60));
    }

    Err(ParseDurationError::InvalidFormat)
}

fn wait_for_connection<'a>(
    quit_flag: &AtomicBool,
    connectors: &'a mut [Box<dyn Connector>],
    sleeper: &dyn Sleeper,
) -> Option<ConnectorGuard<'a>> {
    println!("Waiting for simulator connection...");

    while !quit_flag.load(Ordering::Relaxed) {
        #[allow(clippy::needless_range_loop)]
        // indexed loop used to get mutable reference on a single element, not the whole slice
        for i in 0..connectors.len() {
            if connectors[i].connect() {
                return Some(ConnectorGuard::new(&mut *connectors[i]));
            }
        }
        sleeper.sleep_ms(1000);
    }

    None
}

fn record(
    quit_flag: &AtomicBool,
    fps: u32,
    mut connector: ConnectorGuard,
    saver: &mut Saver<BufWriter<File>>,
    sleeper: &mut dyn Sleeper,
    duration: Option<Duration>,
) -> Result<RecordingFinished, RecordingError> {
    let tick_ms = 1000.0 / fps as f64;
    let mut no_data_count = 0;
    let max_no_data = 20; // disconnect after ~20 frames with no data

    let start = Instant::now();

    while !quit_flag.load(Ordering::Relaxed) {
        if let Some(max_dur) = duration
            && start.elapsed() >= max_dur
        {
            return Ok(RecordingFinished::MaxDurationReached);
        }

        let start = Instant::now();

        match connector.update() {
            Some(data) => {
                no_data_count = 0;
                if let Err(e) = saver.save(&data) {
                    return Err(RecordingError::SavingFrameFailed(e));
                }
            }
            None => {
                no_data_count += 1;
                if no_data_count > max_no_data {
                    return Ok(RecordingFinished::SimDisconnected);
                }
            }
        }

        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        if elapsed_ms < tick_ms {
            sleeper.sleep_ms((tick_ms - elapsed_ms) as u64);
        }
    }

    Ok(RecordingFinished::QuitRequested)
}

pub fn run(
    quit_flag: Arc<AtomicBool>,
    fps: u32,
    max_duration: Option<String>,
) -> Result<RecordingFinished, Error> {
    let mut sleeper = AdaptiveSleeper::default();

    println!("Frames per second: {}", fps);

    let duration = match max_duration {
        None => None,
        Some(ref s) => Some(parse_duration(s)?),
    };

    let mut connectors: Vec<Box<dyn Connector>> = vec![
        Box::new(IRacingConnector::new()),
        Box::new(AssettoCorsaConnector::new()),
    ];

    let connector = wait_for_connection(&quit_flag, &mut connectors, &sleeper);

    let Some(connector) = connector else {
        return Ok(RecordingFinished::QuitRequested);
    };

    let info = connector.info();

    let sim_name = std::str::from_utf8(&info.id).map_err(|_| Error::InvalidSimId)?;
    println!("Connected to: {}", sim_name);

    let filename = generate_filename(sim_name);
    let file = match File::create(&filename) {
        Ok(f) => f,
        Err(e) => {
            return Err(Error::from(RecordError::CreateFileError(e)));
        }
    };

    let writer = BufWriter::new(file);
    let mut saver = match Saver::new(writer, fps as i32, info) {
        Ok(s) => s,
        Err(e) => {
            return Err(Error::from(RecordError::SaverInitError(e)));
        }
    };

    println!("Recording to: {}", filename);
    if let Some(duration) = max_duration {
        println!("Max duration: {}", duration);
    } else {
        println!("Max duration: unlimited (press Ctrl+C to stop)");
    }

    let result = record(
        &quit_flag,
        fps,
        connector,
        &mut saver,
        &mut sleeper,
        duration,
    )?;

    if let Err(e) = saver.flush() {
        return Err(Error::from(RecordError::FlushFailed(e)));
    }

    println!("Recording stopped");
    println!("You can now close this window.");

    Ok(result)
}

fn generate_filename(name: &str) -> String {
    let now = chrono::Local::now();
    format!("ksana_{}_{}.ksr", name, now.format("%Y%m%d_%H_%M_%S"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration_happy() {
        assert_eq!(parse_duration("30s").unwrap(), Duration::from_secs(30));
        assert_eq!(parse_duration("0s").unwrap(), Duration::from_secs(0));
        assert_eq!(parse_duration("12s").unwrap(), Duration::from_secs(12));
        assert_eq!(parse_duration("5m").unwrap(), Duration::from_secs(300));
        assert_eq!(parse_duration("1m").unwrap(), Duration::from_secs(60));
        assert_eq!(parse_duration("10m").unwrap(), Duration::from_secs(600));
    }

    #[test]
    fn test_parse_duration_unhappy() {
        // Empty string
        assert!(matches!(
            parse_duration(""),
            Err(ParseDurationError::InvalidFormat)
        ));

        // No suffix
        assert!(matches!(
            parse_duration("30"),
            Err(ParseDurationError::InvalidFormat)
        ));

        // Invalid suffix
        assert!(matches!(
            parse_duration("30h"),
            Err(ParseDurationError::InvalidFormat)
        ));

        // Invalid number
        assert!(matches!(
            parse_duration("abc"),
            Err(ParseDurationError::InvalidFormat)
        ));

        // Invalid number with valid suffix
        assert!(matches!(
            parse_duration("abcs"),
            Err(ParseDurationError::InvalidFormat)
        ));
    }
}
