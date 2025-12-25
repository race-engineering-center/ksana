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
    fn initialize(&mut self) -> anyhow::Result<()>;
    fn update(&mut self, data: &[u8]) -> anyhow::Result<()>;
    fn stop(&mut self);
}
