use super::shmio::PageWriter;

pub struct Player<W: PageWriter> {
    writer: W,
    payload_version: i32,
}

impl<W: PageWriter> Player<W> {
    pub fn from_writer(writer: W, payload_version: i32) -> Self {
        Self { writer, payload_version }
    }
}

impl<W: PageWriter> crate::Player for Player<W> {
    fn update(&mut self, data: &[u8]) -> anyhow::Result<()> {
        self.writer.update(data, self.payload_version)
    }

    fn stop(&mut self) {
        self.writer.stop()
    }
}
