use super::data::{FrameData, Header, IRSDK_MEMMAPFILENAME, VarHeader};
use crate::Player;
use crate::shm::{EventHandle, SharedMemoryWriter};

const DEFAULT_SHM_SIZE: usize = 1024 * 1024 * 1024;
const IRSDK_DATAVALIDEVENTNAME: &str = "Local\\IRSDKDataValidEvent";

pub struct IRacingPlayer {
    shm: SharedMemoryWriter,
    event: EventHandle,
    payload_version: i32,
}

impl IRacingPlayer {
    pub fn new(payload_version: i32) -> anyhow::Result<Self> {
        let shm = SharedMemoryWriter::create(IRSDK_MEMMAPFILENAME, DEFAULT_SHM_SIZE)?;
        let event = EventHandle::create(IRSDK_DATAVALIDEVENTNAME)?;
        Ok(Self {
            shm,
            event,
            payload_version,
        })
    }
}

impl Player for IRacingPlayer {
    fn update(&mut self, data: &[u8]) -> anyhow::Result<()> {
        let frame = FrameData::deserialize(data, self.payload_version)?;

        let latest_idx = frame.header.latest_buf_index();
        let buf_offset = frame.header.var_buf[latest_idx].buf_offset as usize;

        unsafe {
            // raw telemetry data
            self.shm.write(buf_offset, &frame.raw_data);

            // var headers — only written when present (unchanged frames omit them;
            // SHM already holds the previous values)
            if let Some(var_headers) = &frame.var_headers {
                let var_header_size = std::mem::size_of::<VarHeader>();
                for (i, vh) in var_headers.iter().enumerate() {
                    let vh_bytes = std::slice::from_raw_parts(
                        vh as *const VarHeader as *const u8,
                        var_header_size,
                    );
                    let offset = frame.header.var_header_offset as usize + i * var_header_size;
                    self.shm.write(offset, vh_bytes);
                }
            }

            // session info
            if let Some(session_info) = &frame.session_info {
                let offset = frame.header.session_info_offset as usize;
                self.shm.write(offset, session_info);
            }

            // header last — advancing tick_count is the signal to clients that new data is ready
            let header_bytes = std::slice::from_raw_parts(
                &frame.header as *const Header as *const u8,
                Header::SIZE,
            );
            self.shm.write(0, header_bytes);
        }

        self.event.signal();

        Ok(())
    }

    fn stop(&mut self) {
        unsafe {
            let status_offset = std::mem::offset_of!(Header, status);
            let disconnected: i32 = 0;
            self.shm.write(status_offset, &disconnected.to_le_bytes());
        }
    }
}
