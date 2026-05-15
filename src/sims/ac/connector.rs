use super::shmio::SharedMemoryReader;
use crate::SimInfo;
use crate::sims::ac::data::{AC_OFF, FrameData, GraphicsLike, PhysicsLike, StaticLike};

pub struct Connector<G: GraphicsLike, P: PhysicsLike, S: StaticLike> {
    reader: Option<SharedMemoryReader<G, P, S>>,
    prev_statics: Option<S>,
    graphics_name: &'static str,
    physics_name: &'static str,
    static_name: &'static str,
    sim_id: [u8; 4],
    payload_version: i32,
}

impl<G: GraphicsLike, P: PhysicsLike, S: StaticLike> Connector<G, P, S> {
    pub fn new(
        graphics_name: &'static str,
        physics_name: &'static str,
        static_name: &'static str,
        sim_id: [u8; 4],
        payload_version: i32,
    ) -> Self {
        Self {
            reader: None,
            prev_statics: None,
            graphics_name,
            physics_name,
            static_name,
            sim_id,
            payload_version,
        }
    }
}

impl<G: GraphicsLike, P: PhysicsLike, S: StaticLike> crate::Connector for Connector<G, P, S> {
    fn connect(&mut self) -> bool {
        let reader = match SharedMemoryReader::<G, P, S>::new(
            self.graphics_name,
            self.physics_name,
            self.static_name,
        ) {
            Some(r) => r,
            None => return false,
        };

        let graphics = reader.read_graphics();
        if graphics.status() == AC_OFF {
            return false;
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
        let graphics = reader.read_graphics();

        if graphics.status() == AC_OFF {
            return None;
        }

        let physics = reader.read_physics();
        let statics = reader.read_statics();

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

    fn info(&self) -> SimInfo {
        SimInfo {
            id: self.sim_id,
            payload_version: self.payload_version,
        }
    }
}
