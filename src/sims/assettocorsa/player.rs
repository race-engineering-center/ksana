use super::shm::{AC_GRAPHICS_SHM, AC_PHYSICS_SHM, AC_STATIC_SHM};
use super::shmio::SharedMemoryWriter;
use crate::sims::ac::player::Player as AcPlayer;

pub type AssettoCorsaPlayer = AcPlayer<SharedMemoryWriter>;

impl AssettoCorsaPlayer {
    pub fn new(payload_version: i32) -> anyhow::Result<Self> {
        let writer = SharedMemoryWriter::new(AC_GRAPHICS_SHM, AC_PHYSICS_SHM, AC_STATIC_SHM)
            .ok_or_else(|| anyhow::anyhow!("Failed to initialize shared memory"))?;
        Ok(Self::from_writer(writer, payload_version))
    }
}
