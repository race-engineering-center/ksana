use super::data::{GraphicsLike, PhysicsLike, StaticLike};
use super::shmio::SharedMemoryWriter;

pub struct Player<G: GraphicsLike, P: PhysicsLike, S: StaticLike> {
    writer: SharedMemoryWriter<G, P, S>,
    payload_version: i32,
}

impl<G: GraphicsLike, P: PhysicsLike, S: StaticLike> Player<G, P, S> {
    pub fn from_writer(writer: SharedMemoryWriter<G, P, S>, payload_version: i32) -> Self {
        Self {
            writer,
            payload_version,
        }
    }
}

impl<G: GraphicsLike, P: PhysicsLike, S: StaticLike> crate::Player for Player<G, P, S> {
    fn update(&mut self, data: &[u8]) -> anyhow::Result<()> {
        self.writer.update(data, self.payload_version)
    }

    fn stop(&mut self) {
        self.writer.stop()
    }
}
