use super::data::{AC_OFF, FrameData, StaticPage};
use super::shm::{AC_GRAPHICS_SHM, AC_PHYSICS_SHM, AC_STATIC_SHM};
use super::shmio::AssettoCorsaSharedMemoryReader;
use crate::Connector;
use crate::sims::assettocorsa::data::SHM_SIZE;
use crate::sims::assettocorsa::shmio::SharedMemoryRegionInfo;

pub struct AssettoCorsaConnector {
    reader: Option<AssettoCorsaSharedMemoryReader>,
    prev_statics: Option<StaticPage>,
}

impl AssettoCorsaConnector {
    pub fn new() -> Self {
        Self {
            reader: None,
            prev_statics: None,
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
        let reader = match AssettoCorsaSharedMemoryReader::new(
            SharedMemoryRegionInfo::new(AC_GRAPHICS_SHM, SHM_SIZE),
            SharedMemoryRegionInfo::new(AC_PHYSICS_SHM, SHM_SIZE),
            SharedMemoryRegionInfo::new(AC_STATIC_SHM, SHM_SIZE),
        ) {
            Some(r) => r,
            None => return false,
        };

        match reader.read_graphics() {
            Some(graphics) => {
                if graphics.status == AC_OFF {
                    return false;
                }
            }
            None => return false,
        }

        self.reader = Some(reader);
        true
    }

    fn disconnect(&mut self) {
        self.reader = None;
        self.prev_statics = None;
    }

    fn update(&mut self) -> Option<Vec<u8>> {
        let reader = self.reader.as_ref()?;
        let graphics = reader.read_graphics()?;

        if graphics.status == AC_OFF {
            return None;
        }

        let physics = reader.read_physics()?;
        let statics = reader.read_statics()?;

        let statics_changed = self.prev_statics != Some(statics);
        if statics_changed {
            self.prev_statics = Some(statics);
        }

        let frame = FrameData {
            graphics,
            physics,
            statics: statics_changed.then_some(statics),
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
