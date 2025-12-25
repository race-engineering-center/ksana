use std::fs::File;
use std::io::BufWriter;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

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
}

#[derive(thiserror::Error, Debug)]
pub enum DumpError {
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
    Dump(#[from] DumpError),

    #[error("Invalid simulator ID")]
    InvalidSimId,
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
) -> Result<RecordingFinished, RecordingError> {
    let tick_ms = 1000.0 / fps as f64;
    let mut no_data_count = 0;
    let max_no_data = 20; // disconnect after ~20 frames with no data

    while !quit_flag.load(Ordering::Relaxed) {
        let start = std::time::Instant::now();

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

pub fn run(quit_flag: Arc<AtomicBool>, fps: u32) -> Result<RecordingFinished, Error> {
    let mut sleeper = AdaptiveSleeper::default();

    println!("Frames per second: {}", fps);

    let mut connectors: Vec<Box<dyn Connector>> = vec![
        Box::new(IRacingConnector::new()),
        Box::new(AssettoCorsaConnector::new()),
    ];

    let connector = wait_for_connection(&quit_flag, &mut connectors, &sleeper);

    let Some(connector) = connector else {
        return Ok(RecordingFinished::QuitRequested);
    };

    let id = connector.id();

    let sim_name = std::str::from_utf8(&id).map_err(|_| Error::InvalidSimId)?;
    println!("Connected to: {}", sim_name);

    let filename = generate_filename(sim_name);
    let file = match File::create(&filename) {
        Ok(f) => f,
        Err(e) => {
            return Err(Error::from(DumpError::CreateFileError(e)));
        }
    };

    let writer = BufWriter::new(file);
    let mut saver = match Saver::new(writer, fps as i32, id) {
        Ok(s) => s,
        Err(e) => {
            return Err(Error::from(DumpError::SaverInitError(e)));
        }
    };

    println!("Recording to: {}", filename);
    let result = record(&quit_flag, fps, connector, &mut saver, &mut sleeper)?;

    if let Err(e) = saver.flush() {
        return Err(Error::from(DumpError::FlushFailed(e)));
    }

    println!("Recording stopped");
    println!("You can now close this window.");

    Ok(result)
}

fn generate_filename(name: &str) -> String {
    let now = chrono::Local::now();
    format!("ksana_{}_{}.bin", name, now.format("%Y%m%d_%H_%M_%S"))
}
