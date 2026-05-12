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
    writer: AssettoCorsaSharedMemoryWriter,
    payload_version: i32,
}

impl AssettoCorsaPlayer {
    pub fn new(payload_version: i32) -> anyhow::Result<Self> {
        let writer = AssettoCorsaSharedMemoryWriter::new(
            SharedMemoryRegionInfo::new(AC_GRAPHICS_SHM, SHM_SIZE),
            SharedMemoryRegionInfo::new(AC_PHYSICS_SHM, SHM_SIZE),
            SharedMemoryRegionInfo::new(AC_STATIC_SHM, SHM_SIZE),
        )
        .ok_or(AssettoCorsaError::InitializationFailed)?;
        Ok(Self {
            writer,
            payload_version,
        })
    }
}

impl Player for AssettoCorsaPlayer {
    fn update(&mut self, data: &[u8]) -> anyhow::Result<()> {
        self.writer.update(data, self.payload_version)
    }

    fn stop(&mut self) {
        self.writer.stop()
    }
}
