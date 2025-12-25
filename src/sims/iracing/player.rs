use super::data::{FrameData, Header, IRSDK_MEMMAPFILENAME, VarHeader};
use crate::Player;
use crate::shm::{EventHandle, SharedMemoryWriter};

const DEFAULT_SHM_SIZE: usize = 1024 * 1024 * 1024;
const IRSDK_DATAVALIDEVENTNAME: &str = "Local\\IRSDKDataValidEvent";

#[derive(thiserror::Error, Debug)]
enum IRacingError {
    #[error("Failed to initialize shared memory or event")]
    InitializationFailed,
}

pub struct IRacingPlayer {
    shm: Option<SharedMemoryWriter>,
    event: Option<EventHandle>,
}

impl IRacingPlayer {
    pub fn new() -> Self {
        Self {
            shm: None,
            event: None,
        }
    }
}

impl Default for IRacingPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl Player for IRacingPlayer {
    fn initialize(&mut self) -> anyhow::Result<()> {
        let shm = SharedMemoryWriter::create(IRSDK_MEMMAPFILENAME, DEFAULT_SHM_SIZE)?;
        let event = EventHandle::create(IRSDK_DATAVALIDEVENTNAME)?;
        self.shm = Some(shm);
        self.event = Some(event);
        Ok(())
    }

    fn update(&mut self, data: &[u8]) -> anyhow::Result<()> {
        let shm = self
            .shm
            .as_mut()
            .ok_or(IRacingError::InitializationFailed)?;

        let frame = FrameData::deserialize(data)?;

        let latest_idx = frame.header.latest_buf_index();
        let buf_offset = frame.header.var_buf[latest_idx].buf_offset as usize;

        unsafe {
            // header
            let header_bytes = std::slice::from_raw_parts(
                &frame.header as *const Header as *const u8,
                Header::SIZE,
            );
            shm.write(0, header_bytes);

            // raw telemetry data
            shm.write(buf_offset, &frame.raw_data);

            // var headers
            let var_header_size = std::mem::size_of::<VarHeader>();
            for (i, vh) in frame.var_headers.iter().enumerate() {
                let vh_bytes = std::slice::from_raw_parts(
                    vh as *const VarHeader as *const u8,
                    var_header_size,
                );
                let offset = frame.header.var_header_offset as usize + i * var_header_size;
                shm.write(offset, vh_bytes);
            }

            // session info
            if frame.session_info.is_some() {
                shm.write(
                    frame.header.session_info_offset as usize,
                    #[allow(clippy::unwrap_used)] // safe because we checked for none above
                    frame.session_info.as_ref().unwrap().as_bytes(),
                );
            }
        }

        Ok(())
    }

    fn stop(&mut self) {
        if let Some(ref mut shm) = self.shm {
            unsafe {
                let status_offset = std::mem::offset_of!(Header, status);
                let disconnected: i32 = 0;
                shm.write(status_offset, &disconnected.to_ne_bytes());
            }
        }

        self.shm = None;
        self.event = None;
    }
}
