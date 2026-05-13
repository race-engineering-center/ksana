use crate::io::IOError;

pub trait Sleeper {
    fn sleep_ms(&self, ms: u64);
}

#[derive(Debug, Copy, Clone)]
pub struct SimInfo {
    pub id: [u8; 4],
    pub payload_version: i32,
}

pub trait Connector {
    fn connect(&mut self) -> bool;
    fn disconnect(&mut self);
    fn update(&mut self) -> Option<Vec<u8>>;
    fn info(&self) -> SimInfo;
}

pub trait Player {
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

    #[error("Failed to create player: {0}")]
    FailedToCreatePlayer(anyhow::Error),

    #[error("Failed to load frame: {0}")]
    FailedToLoadFrame(IOError),

    #[error("Failed to update player: {0}")]
    FailedToUpdatePlayer(anyhow::Error),
}
