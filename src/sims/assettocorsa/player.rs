use super::data::{GraphicsPage, PhysicsPage, StaticPage};
use super::shm::{AC_GRAPHICS_SHM, AC_PHYSICS_SHM, AC_STATIC_SHM};
use crate::sims::ac::player::Player as AcPlayer;
use crate::sims::ac::shmio::SharedMemoryWriter;

pub type AssettoCorsaPlayer = AcPlayer<GraphicsPage, PhysicsPage, StaticPage>;

impl AssettoCorsaPlayer {
    pub fn new(payload_version: i32) -> anyhow::Result<Self> {
        let writer = SharedMemoryWriter::<GraphicsPage, PhysicsPage, StaticPage>::new(
            AC_GRAPHICS_SHM,
            AC_PHYSICS_SHM,
            AC_STATIC_SHM,
        )
        .ok_or_else(|| anyhow::anyhow!("Failed to initialize shared memory"))?;
        Ok(Self::from_writer(writer, payload_version))
    }
}
