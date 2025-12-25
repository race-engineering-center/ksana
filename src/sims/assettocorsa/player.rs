use super::data::{
    AC_GRAPHICS_SHM, AC_OFF, AC_PHYSICS_SHM, AC_STATIC_SHM, FrameData, GraphicsPage, PhysicsPage,
    StaticPage,
};
use crate::Player;
use crate::shm::SharedMemoryWriter;
use crate::sims::assettocorsa::data::SHM_SIZE;

#[derive(thiserror::Error, Debug)]
enum AssettoCorsaError {
    #[error("Failed to initialize shared memory")]
    InitializationFailed,
}

pub struct AssettoCorsaPlayer {
    graphics_shm: Option<SharedMemoryWriter>,
    physics_shm: Option<SharedMemoryWriter>,
    static_shm: Option<SharedMemoryWriter>,
}

impl AssettoCorsaPlayer {
    pub fn new() -> Self {
        Self {
            graphics_shm: None,
            physics_shm: None,
            static_shm: None,
        }
    }
}

impl Default for AssettoCorsaPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl Player for AssettoCorsaPlayer {
    fn initialize(&mut self) -> anyhow::Result<()> {
        let graphics = SharedMemoryWriter::create(AC_GRAPHICS_SHM, SHM_SIZE)?;
        let physics = SharedMemoryWriter::create(AC_PHYSICS_SHM, SHM_SIZE)?;
        let statics = SharedMemoryWriter::create(AC_STATIC_SHM, SHM_SIZE)?;

        self.graphics_shm = Some(graphics);
        self.physics_shm = Some(physics);
        self.static_shm = Some(statics);

        Ok(())
    }

    fn update(&mut self, data: &[u8]) -> anyhow::Result<()> {
        let graphics_shm = self
            .graphics_shm
            .as_mut()
            .ok_or(AssettoCorsaError::InitializationFailed)?;
        let physics_shm = self
            .physics_shm
            .as_mut()
            .ok_or(AssettoCorsaError::InitializationFailed)?;
        let static_shm = self
            .static_shm
            .as_mut()
            .ok_or(AssettoCorsaError::InitializationFailed)?;

        // deserialize the frame data
        let frame = FrameData::deserialize(data)?;

        unsafe {
            // graphics
            let graphics_bytes = std::slice::from_raw_parts(
                &frame.graphics as *const GraphicsPage as *const u8,
                std::mem::size_of::<GraphicsPage>(),
            );
            graphics_shm.write(0, graphics_bytes);

            // physics
            let physics_bytes = std::slice::from_raw_parts(
                &frame.physics as *const PhysicsPage as *const u8,
                std::mem::size_of::<PhysicsPage>(),
            );
            physics_shm.write(0, physics_bytes);

            // static
            let statics_bytes = std::slice::from_raw_parts(
                &frame.statics as *const StaticPage as *const u8,
                std::mem::size_of::<StaticPage>(),
            );
            static_shm.write(0, statics_bytes);
        }

        Ok(())
    }

    fn stop(&mut self) {
        if let Some(ref mut shm) = self.graphics_shm {
            unsafe {
                let status_offset = std::mem::offset_of!(GraphicsPage, status);
                shm.write(status_offset, &AC_OFF.to_ne_bytes());
            }
        }

        self.graphics_shm = None;
        self.physics_shm = None;
        self.static_shm = None;
    }
}
