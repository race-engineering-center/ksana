use std::marker::PhantomData;

use crate::shm::SharedMemoryReader as ShmReader;
use crate::shm::SharedMemoryWriter as ShmWriter;
use crate::sims::ac::data::FrameData;

use super::data::{GraphicsLike, PhysicsLike, StaticLike};

pub struct SharedMemoryReader<G: GraphicsLike, P: PhysicsLike, S: StaticLike> {
    graphics_shm: ShmReader,
    physics_shm: ShmReader,
    static_shm: ShmReader,
    _phantom_g: PhantomData<G>,
    _phantom_p: PhantomData<P>,
    _phantom_s: PhantomData<S>,
}

impl<G: GraphicsLike, P: PhysicsLike, S: StaticLike> SharedMemoryReader<G, P, S> {
    pub fn new(graphics_name: &str, physics_name: &str, static_name: &str) -> Option<Self> {
        let graphics = ShmReader::open(graphics_name, size_of::<G>()).ok()?;
        let physics = ShmReader::open(physics_name, size_of::<P>()).ok()?;
        let statics = ShmReader::open(static_name, size_of::<S>()).ok()?;

        Some(Self {
            graphics_shm: graphics,
            physics_shm: physics,
            static_shm: statics,
            _phantom_g: PhantomData,
            _phantom_p: PhantomData,
            _phantom_s: PhantomData,
        })
    }

    pub fn read_graphics(&self) -> Option<G> {
        unsafe {
            let ptr = self.graphics_shm.as_ptr() as *const G;
            Some(std::ptr::read(ptr))
        }
    }

    pub fn read_physics(&self) -> Option<P> {
        unsafe {
            let ptr = self.physics_shm.as_ptr() as *const P;
            Some(std::ptr::read(ptr))
        }
    }

    pub fn read_statics(&self) -> Option<S> {
        unsafe {
            let ptr = self.static_shm.as_ptr() as *const S;
            Some(std::ptr::read(ptr))
        }
    }
}

pub struct SharedMemoryWriter<G: GraphicsLike, P: PhysicsLike, S: StaticLike> {
    graphics_shm: Option<ShmWriter>,
    physics_shm: Option<ShmWriter>,
    static_shm: Option<ShmWriter>,
    _phantom_g: PhantomData<G>,
    _phantom_p: PhantomData<P>,
    _phantom_s: PhantomData<S>,
}

impl<G: GraphicsLike, P: PhysicsLike, S: StaticLike> SharedMemoryWriter<G, P, S> {
    pub fn new(graphics_name: &str, physics_name: &str, static_name: &str) -> Option<Self> {
        let graphics = ShmWriter::create(graphics_name, size_of::<G>()).ok()?;
        let physics = ShmWriter::create(physics_name, size_of::<P>()).ok()?;
        let statics = ShmWriter::create(static_name, size_of::<S>()).ok()?;

        Some(Self {
            graphics_shm: Some(graphics),
            physics_shm: Some(physics),
            static_shm: Some(statics),
            _phantom_g: PhantomData,
            _phantom_p: PhantomData,
            _phantom_s: PhantomData,
        })
    }

    pub fn update(&mut self, data: &[u8], payload_version: i32) -> anyhow::Result<()> {
        let graphics_shm = self
            .graphics_shm
            .as_mut()
            .expect("Graphics not initialized");
        let physics_shm = self.physics_shm.as_mut().expect("Physics not initialized");

        let frame = FrameData::<G, P, S>::deserialize(data, payload_version)?;

        unsafe {
            // graphics
            let graphics_bytes = std::slice::from_raw_parts(
                &frame.graphics as *const G as *const u8,
                std::mem::size_of::<G>(),
            );
            graphics_shm.write(0, graphics_bytes);

            // physics
            let physics_bytes = std::slice::from_raw_parts(
                &frame.physics as *const P as *const u8,
                std::mem::size_of::<P>(),
            );
            physics_shm.write(0, physics_bytes);

            // static might not be present, write conditionally
            if let Some(statics) = &frame.statics {
                let static_shm = self.static_shm.as_mut().expect("Static not initialized");

                let statics_bytes = std::slice::from_raw_parts(
                    statics as *const S as *const u8,
                    std::mem::size_of::<S>(),
                );
                static_shm.write(0, statics_bytes);
            }
        }
        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(ref mut shm) = self.graphics_shm {
            unsafe {
                shm.write(
                    super::data::GRAPHICS_STATUS_OFFSET,
                    &super::data::AC_OFF.to_le_bytes(),
                );
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

    use crate::sims::ac::data::{AC_OFF, GraphicsPage, PhysicsPage, StaticPage};

    type TestGraphics = GraphicsPage<1024>;
    type TestPhysics = PhysicsPage<512>;
    type TestStatic = StaticPage<256>;

    type FrameData = crate::sims::ac::data::FrameData<TestGraphics, TestPhysics, TestStatic>;

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
        use crate::sims::ac::shmio::{SharedMemoryReader, SharedMemoryWriter};

        let id = generate_id().to_string();

        let mut writer = SharedMemoryWriter::<TestGraphics, TestPhysics, TestStatic>::new(
            &format!("{}-graphics", id),
            &format!("{}-physics", id),
            &format!("{}-static", id),
        )
        .unwrap();

        let reader = SharedMemoryReader::<TestGraphics, TestPhysics, TestStatic>::new(
            &format!("{}-graphics", id),
            &format!("{}-physics", id),
            &format!("{}-static", id),
        )
        .unwrap();

        let mut frame = FrameData::default();
        frame.physics.content = [42; 512];
        frame.graphics.packet_id = 123;
        frame.graphics.status = 5;
        frame.graphics.content = [7; 1024];
        frame.statics = Some(StaticPage { content: [99; 256] });

        let data = frame.serialize();
        writer.update(&data, 2).unwrap();

        let graphics = reader.read_graphics().unwrap();
        let physics = reader.read_physics().unwrap();
        let statics = reader.read_statics().unwrap();

        assert_eq!(graphics, frame.graphics);
        assert_eq!(physics, frame.physics);
        assert_eq!(statics, frame.statics.unwrap());

        // write another frame with different data without statics
        let mut second_frame = FrameData::default();
        second_frame.physics.content = [73; 512];
        second_frame.graphics.packet_id = 124;
        second_frame.graphics.status = 6;
        second_frame.graphics.content = [9; 1024];

        let data = second_frame.serialize();
        writer.update(&data, 2).unwrap();

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
