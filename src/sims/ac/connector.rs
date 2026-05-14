use crate::SimInfo;
use crate::sims::ac::data::{AC_OFF, FrameData, GraphicsLike};

use super::shmio::PageReader;

pub struct Connector<R: PageReader> {
    reader: Option<R>,
    prev_statics: Option<R::Static>,
    graphics_name: &'static str,
    physics_name: &'static str,
    static_name: &'static str,
    sim_id: [u8; 4],
    payload_version: i32,
}

impl<R: PageReader> Connector<R> {
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

impl<R: PageReader> crate::Connector for Connector<R> {
    fn connect(&mut self) -> bool {
        let reader = match R::new(self.graphics_name, self.physics_name, self.static_name) {
            Some(r) => r,
            None => return false,
        };

        match reader.read_graphics() {
            Some(graphics) => {
                if graphics.status() == AC_OFF {
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

        if graphics.status() == AC_OFF {
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

    fn info(&self) -> SimInfo {
        SimInfo {
            id: self.sim_id,
            payload_version: self.payload_version,
        }
    }
}
