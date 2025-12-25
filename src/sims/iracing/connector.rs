use super::data::{FrameData, Header, IRSDK_MEMMAPFILENAME, VarHeader};
use crate::Connector;
use crate::shm::SharedMemoryReader;

const DEFAULT_SHM_SIZE: usize = 1024 * 1024 * 32;

pub struct IRacingConnector {
    shm: Option<SharedMemoryReader>,
    last_session_info_update: i32,
    last_tick_count: i32,
}

impl IRacingConnector {
    pub fn new() -> Self {
        Self {
            shm: None,
            last_session_info_update: 0,
            last_tick_count: 0,
        }
    }

    fn read_header(&self) -> Option<Header> {
        let shm = self.shm.as_ref()?;
        unsafe {
            let ptr = shm.as_ptr() as *const Header;
            Some(std::ptr::read(ptr))
        }
    }

    fn read_var_headers(&self, header: &Header) -> Vec<VarHeader> {
        let shm = match self.shm.as_ref() {
            Some(s) => s,
            None => return vec![],
        };

        let mut var_headers = Vec::with_capacity(header.num_vars as usize);
        let base_ptr = shm.as_ptr();

        for i in 0..header.num_vars {
            unsafe {
                let vh_ptr = base_ptr.add(header.var_header_offset as usize) as *const VarHeader;
                let vh_ptr = vh_ptr.add(i as usize);
                var_headers.push(std::ptr::read(vh_ptr));
            }
        }

        var_headers
    }

    fn read_session_info(&self, header: &Header) -> String {
        let shm = self
            .shm
            .as_ref()
            .expect("Shared memory reader should be connected");

        unsafe {
            let ptr = shm.as_ptr().add(header.session_info_offset as usize);
            let slice = std::slice::from_raw_parts(ptr, header.session_info_len as usize);

            // Find null terminator
            let len = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
            String::from_utf8_lossy(&slice[..len]).to_string()
        }
    }

    fn read_raw_data(&self, header: &Header) -> Vec<u8> {
        let shm = self
            .shm
            .as_ref()
            .expect("Shared memory reader should be connected");

        let latest_idx = header.latest_buf_index();
        let buf_offset = header.var_buf[latest_idx].buf_offset as usize;

        unsafe {
            let ptr = shm.as_ptr().add(buf_offset);
            let slice = std::slice::from_raw_parts(ptr, header.buf_len as usize);
            slice.to_vec()
        }
    }
}

impl Default for IRacingConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl Connector for IRacingConnector {
    fn connect(&mut self) -> bool {
        match SharedMemoryReader::open(IRSDK_MEMMAPFILENAME, DEFAULT_SHM_SIZE) {
            Ok(shm) => {
                let ptr = shm.as_ptr() as *const Header;
                let header = unsafe { std::ptr::read(ptr) };

                if header.is_connected() {
                    self.shm = Some(shm);
                    self.last_session_info_update = 0;
                    self.last_tick_count = 0;
                    true
                } else {
                    false
                }
            }
            Err(_) => false,
        }
    }

    fn disconnect(&mut self) {
        self.shm = None;
        self.last_session_info_update = 0;
        self.last_tick_count = 0;
    }

    fn update(&mut self) -> Option<Vec<u8>> {
        let header = self.read_header()?;

        if !header.is_connected() {
            return None;
        }

        let latest_idx = header.latest_buf_index();
        let current_tick = header.var_buf[latest_idx].tick_count;

        if current_tick == self.last_tick_count {
            // No new data
            return None;
        }
        self.last_tick_count = current_tick;

        // var headers
        let var_headers = self.read_var_headers(&header);

        // session info
        let session_info = if header.session_info_update != self.last_session_info_update {
            self.last_session_info_update = header.session_info_update;
            Some(self.read_session_info(&header))
        } else {
            None
        };

        // data
        let raw_data = self.read_raw_data(&header);

        // serialize frame
        let frame = FrameData {
            header,
            var_headers,
            session_info,
            raw_data,
        };

        frame.serialize()
    }

    fn id(&self) -> [u8; 4] {
        *b"irac"
    }
}
