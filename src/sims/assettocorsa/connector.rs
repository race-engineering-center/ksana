use super::data::{
    AC_GRAPHICS_SHM, AC_OFF, AC_PHYSICS_SHM, AC_STATIC_SHM, FrameData, GraphicsPage, PhysicsPage,
    StaticPage,
};
use crate::Connector;
use crate::shm::SharedMemoryReader;
use crate::sims::assettocorsa::data::SHM_SIZE;

pub struct AssettoCorsaConnector {
    graphics_shm: Option<SharedMemoryReader>,
    physics_shm: Option<SharedMemoryReader>,
    static_shm: Option<SharedMemoryReader>,
}

impl AssettoCorsaConnector {
    pub fn new() -> Self {
        Self {
            graphics_shm: None,
            physics_shm: None,
            static_shm: None,
        }
    }

    fn read_graphics(&self) -> Option<GraphicsPage> {
        let shm = self.graphics_shm.as_ref()?;
        unsafe {
            let ptr = shm.as_ptr() as *const GraphicsPage;
            Some(std::ptr::read(ptr))
        }
    }

    fn read_physics(&self) -> Option<PhysicsPage> {
        let shm = self.physics_shm.as_ref()?;
        unsafe {
            let ptr = shm.as_ptr() as *const PhysicsPage;
            Some(std::ptr::read(ptr))
        }
    }

    fn read_statics(&self) -> Option<StaticPage> {
        let shm = self.static_shm.as_ref()?;
        unsafe {
            let ptr = shm.as_ptr() as *const StaticPage;
            Some(std::ptr::read(ptr))
        }
    }
}

impl Default for AssettoCorsaConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl Connector for AssettoCorsaConnector {
    fn connect(&mut self) -> bool {
        let graphics = match SharedMemoryReader::open(AC_GRAPHICS_SHM, SHM_SIZE) {
            Ok(shm) => shm,
            Err(_) => return false,
        };

        let physics = match SharedMemoryReader::open(AC_PHYSICS_SHM, SHM_SIZE) {
            Ok(shm) => shm,
            Err(_) => return false,
        };

        let statics = match SharedMemoryReader::open(AC_STATIC_SHM, SHM_SIZE) {
            Ok(shm) => shm,
            Err(_) => return false,
        };

        unsafe {
            let graphics_ptr = graphics.as_ptr() as *const GraphicsPage;
            let graphics_page = std::ptr::read(graphics_ptr);

            if graphics_page.status == AC_OFF {
                return false;
            }
        }

        self.graphics_shm = Some(graphics);
        self.physics_shm = Some(physics);
        self.static_shm = Some(statics);

        true
    }

    fn disconnect(&mut self) {
        self.graphics_shm = None;
        self.physics_shm = None;
        self.static_shm = None;
    }

    fn update(&mut self) -> Option<Vec<u8>> {
        let graphics = self.read_graphics()?;

        if graphics.status == AC_OFF {
            return None;
        }

        let physics = self.read_physics()?;
        let statics = self.read_statics()?;

        let frame = FrameData {
            graphics,
            physics,
            statics,
        };

        Some(frame.serialize())
    }

    fn id(&self) -> [u8; 4] {
        *b"acsa"
    }
}

#[cfg(test)]
mod tests {
    use super::super::data::SHM_SIZE;
    use super::super::data::{GraphicsPage, PhysicsPage, StaticPage};

    #[test]
    fn test_size_constraints() {
        assert!(SHM_SIZE >= size_of::<PhysicsPage>());
        assert!(SHM_SIZE >= size_of::<GraphicsPage>());
        assert!(SHM_SIZE >= size_of::<StaticPage>());
    }
}
