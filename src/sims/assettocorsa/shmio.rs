use crate::shm::{SharedMemoryReader, SharedMemoryWriter};
use crate::sims::assettocorsa::data::{AC_OFF, FrameData, GraphicsPage, PhysicsPage, StaticPage};

pub struct SharedMemoryRegionInfo {
    name: String,
    size: usize,
}

impl SharedMemoryRegionInfo {
    pub fn new(name: &str, size: usize) -> Self {
        Self {
            name: name.to_string(),
            size,
        }
    }
}

pub struct AssettoCorsaSharedMemoryReader {
    graphics_shm: Option<SharedMemoryReader>,
    physics_shm: Option<SharedMemoryReader>,
    static_shm: Option<SharedMemoryReader>,
}

impl AssettoCorsaSharedMemoryReader {
    pub fn new(
        graphics_info: SharedMemoryRegionInfo,
        physics_info: SharedMemoryRegionInfo,
        statics_info: SharedMemoryRegionInfo,
    ) -> Option<Self> {
        let graphics = SharedMemoryReader::open(&graphics_info.name, graphics_info.size).ok()?;
        let physics = SharedMemoryReader::open(&physics_info.name, physics_info.size).ok()?;
        let statics = SharedMemoryReader::open(&statics_info.name, statics_info.size).ok()?;

        Some(Self {
            graphics_shm: Some(graphics),
            physics_shm: Some(physics),
            static_shm: Some(statics),
        })
    }

    pub fn read_graphics(&self) -> Option<GraphicsPage> {
        let shm = self.graphics_shm.as_ref()?;
        unsafe {
            let ptr = shm.as_ptr() as *const GraphicsPage;
            Some(std::ptr::read(ptr))
        }
    }

    pub fn read_physics(&self) -> Option<PhysicsPage> {
        let shm = self.physics_shm.as_ref()?;
        unsafe {
            let ptr = shm.as_ptr() as *const PhysicsPage;
            Some(std::ptr::read(ptr))
        }
    }

    pub fn read_statics(&self) -> Option<StaticPage> {
        let shm = self.static_shm.as_ref()?;
        unsafe {
            let ptr = shm.as_ptr() as *const StaticPage;
            Some(std::ptr::read(ptr))
        }
    }
}

pub struct AssettoCorsaSharedMemoryWriter {
    graphics_shm: Option<SharedMemoryWriter>,
    physics_shm: Option<SharedMemoryWriter>,
    static_shm: Option<SharedMemoryWriter>,
}

impl AssettoCorsaSharedMemoryWriter {
    pub fn new(
        graphics_info: SharedMemoryRegionInfo,
        physics_info: SharedMemoryRegionInfo,
        statics_info: SharedMemoryRegionInfo,
    ) -> Option<Self> {
        let graphics = SharedMemoryWriter::create(&graphics_info.name, graphics_info.size).ok()?;
        let physics = SharedMemoryWriter::create(&physics_info.name, physics_info.size).ok()?;
        let statics = SharedMemoryWriter::create(&statics_info.name, statics_info.size).ok()?;

        Some(Self {
            graphics_shm: Some(graphics),
            physics_shm: Some(physics),
            static_shm: Some(statics),
        })
    }

    pub fn update(&mut self, data: &[u8]) -> anyhow::Result<()> {
        let graphics_shm = self
            .graphics_shm
            .as_mut()
            .expect("Graphics not initialized");
        let physics_shm = self.physics_shm.as_mut().expect("Physics not initialized");

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

            // static might not be present, write conditionally
            if let Some(statics) = &frame.statics {
                let static_shm = self.static_shm.as_mut().expect("Static not initialized");

                let statics_bytes = std::slice::from_raw_parts(
                    statics as *const StaticPage as *const u8,
                    std::mem::size_of::<StaticPage>(),
                );
                static_shm.write(0, statics_bytes);
            }
        }
        Ok(())
    }

    pub fn stop(&mut self) {
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

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    fn generate_id() -> u64 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos() as u64;

        let pid = std::process::id() as u64;
        nanos ^ (pid << 32)
    }

    #[test]
    #[cfg(not(miri))]
    fn test_read_write() {
        use super::*;

        let id = generate_id().to_string();

        let mut writer = AssettoCorsaSharedMemoryWriter::new(
            SharedMemoryRegionInfo {
                name: format!("{}-graphics", id),
                size: 2048,
            },
            SharedMemoryRegionInfo {
                name: format!("{}-physics", id),
                size: 1024,
            },
            SharedMemoryRegionInfo {
                name: format!("{}-static", id),
                size: 2048,
            },
        )
        .unwrap();

        let reader = AssettoCorsaSharedMemoryReader::new(
            SharedMemoryRegionInfo {
                name: format!("{}-graphics", id),
                size: 2048,
            },
            SharedMemoryRegionInfo {
                name: format!("{}-physics", id),
                size: 1024,
            },
            SharedMemoryRegionInfo {
                name: format!("{}-static", id),
                size: 2048,
            },
        )
        .unwrap();

        let mut frame = FrameData::default();
        frame.physics.content = [42; 1024];
        frame.graphics.packet_id = 123;
        frame.graphics.status = 5;
        frame.graphics.content = [7; 2040];
        frame.statics = Some(StaticPage {
            sm_version: [1; 15],
            ac_version: [2; 15],
            content: [99; 1988],
        });

        let data = frame.serialize();
        writer.update(&data).unwrap();

        let graphics = reader.read_graphics().unwrap();
        let physics = reader.read_physics().unwrap();
        let statics = reader.read_statics().unwrap();

        assert_eq!(graphics, frame.graphics);
        assert_eq!(physics, frame.physics);
        assert_eq!(statics, frame.statics.unwrap());

        // write another frame with different data without statics
        let mut second_frame = FrameData::default();
        second_frame.physics.content = [73; 1024];
        second_frame.graphics.packet_id = 124;
        second_frame.graphics.status = 6;
        second_frame.graphics.content = [9; 2040];

        let data = second_frame.serialize();
        writer.update(&data).unwrap();

        let graphics = reader.read_graphics().unwrap();
        let physics = reader.read_physics().unwrap();
        let statics = reader.read_statics().unwrap();

        assert_eq!(graphics, second_frame.graphics);
        assert_eq!(physics, second_frame.physics);
        assert_eq!(statics, frame.statics.unwrap()); // statics should remain unchanged

        // stop the writer and verify that graphics sees AC_OFF
        writer.stop();

        let graphics = reader.read_graphics().unwrap();
        assert_eq!(graphics.status, AC_OFF);
    }
}
