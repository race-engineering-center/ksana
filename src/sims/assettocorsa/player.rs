use super::shm::{AC_GRAPHICS_SHM, AC_PHYSICS_SHM, AC_STATIC_SHM};
use super::shmio::AssettoCorsaSharedMemoryWriter;
use crate::Player;
use crate::sims::assettocorsa::data::SHM_SIZE;
use crate::sims::assettocorsa::shmio::SharedMemoryRegionInfo;

#[derive(thiserror::Error, Debug)]
enum AssettoCorsaError {
    #[error("Failed to initialize shared memory")]
    InitializationFailed,
}

pub struct AssettoCorsaPlayer {
    writer: Option<AssettoCorsaSharedMemoryWriter>,
}

impl AssettoCorsaPlayer {
    pub fn new() -> Self {
        Self { writer: None }
    }
}

impl Default for AssettoCorsaPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl Player for AssettoCorsaPlayer {
    fn initialize(&mut self, _file_version: i32) -> anyhow::Result<()> {
        let writer = AssettoCorsaSharedMemoryWriter::new(
            SharedMemoryRegionInfo::new(AC_GRAPHICS_SHM, SHM_SIZE),
            SharedMemoryRegionInfo::new(AC_PHYSICS_SHM, SHM_SIZE),
            SharedMemoryRegionInfo::new(AC_STATIC_SHM, SHM_SIZE),
        )
        .ok_or(AssettoCorsaError::InitializationFailed)?;
        self.writer = Some(writer);
        Ok(())
    }

    fn update(&mut self, data: &[u8]) -> anyhow::Result<()> {
        let writer = self.writer.as_mut().expect("Player not initialized");
        writer.update(data)
    }

    fn stop(&mut self) {
        let writer = self.writer.as_mut().expect("Player not initialized");
        writer.stop()
    }
}
