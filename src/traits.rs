use crate::io::IOError;

pub trait Sleeper {
    fn sleep_ms(&self, ms: u64);
}

pub trait Connector {
    fn connect(&mut self) -> bool;
    fn disconnect(&mut self);
    fn update(&mut self) -> Option<Vec<u8>>;
    fn id(&self) -> [u8; 4];
}

pub trait Player {
    fn initialize(&mut self, file_version: i32) -> anyhow::Result<()>;
    fn update(&mut self, data: &[u8]) -> anyhow::Result<()>;
    fn stop(&mut self);
}

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
