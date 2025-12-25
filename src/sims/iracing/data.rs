use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{self, Cursor, Read};

pub const IRSDK_MAX_BUFS: usize = 4;
pub const IRSDK_MAX_STRING: usize = 32;
pub const IRSDK_MAX_DESC: usize = 64;

pub const IRSDK_MEMMAPFILENAME: &str = "Local\\IRSDKMemMapFileName";

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusField {
    Connected = 1,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VarBuf {
    pub tick_count: i32,
    pub buf_offset: i32,
    pub pad: [i32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VarHeader {
    pub var_type: i32,
    pub offset: i32,
    pub count: i32,
    pub count_as_time: u8,
    pub pad: [u8; 3],
    pub name: [u8; IRSDK_MAX_STRING],
    pub desc: [u8; IRSDK_MAX_DESC],
    pub unit: [u8; IRSDK_MAX_STRING],
}

impl Default for VarHeader {
    fn default() -> Self {
        Self {
            var_type: 0,
            offset: 0,
            count: 0,
            count_as_time: 0,
            pad: [0; 3],
            name: [0; IRSDK_MAX_STRING],
            desc: [0; IRSDK_MAX_DESC],
            unit: [0; IRSDK_MAX_STRING],
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Header {
    pub ver: i32,
    pub status: i32,
    pub tick_rate: i32,

    pub session_info_update: i32,
    pub session_info_len: i32,
    pub session_info_offset: i32,

    pub num_vars: i32,
    pub var_header_offset: i32,

    pub num_buf: i32,
    pub buf_len: i32,
    pub pad1: [i32; 2],
    pub var_buf: [VarBuf; IRSDK_MAX_BUFS],
}

impl Default for Header {
    fn default() -> Self {
        Self {
            ver: 0,
            status: 0,
            tick_rate: 0,
            session_info_update: 0,
            session_info_len: 0,
            session_info_offset: 0,
            num_vars: 0,
            var_header_offset: 0,
            num_buf: 0,
            buf_len: 0,
            pad1: [0; 2],
            var_buf: [VarBuf::default(); IRSDK_MAX_BUFS],
        }
    }
}

impl Header {
    pub const SIZE: usize = std::mem::size_of::<Self>();

    pub fn is_connected(&self) -> bool {
        (self.status & StatusField::Connected as i32) != 0
    }

    pub fn latest_buf_index(&self) -> usize {
        let mut latest = 0;
        for i in 1..self.num_buf as usize {
            if self.var_buf[i].tick_count > self.var_buf[latest].tick_count {
                latest = i;
            }
        }
        latest
    }
}

// Frame data and serialization

#[derive(Debug, Clone)]
pub struct FrameData {
    pub header: Header,
    pub var_headers: Vec<VarHeader>,
    pub session_info: Option<String>,
    pub raw_data: Vec<u8>,
}

impl FrameData {
    pub fn serialize(&self) -> Option<Vec<u8>> {
        let mut buffer = Vec::new();

        // main header
        let header_bytes = unsafe {
            std::slice::from_raw_parts(&self.header as *const _ as *const u8, Header::SIZE)
        };
        buffer.extend_from_slice(header_bytes);

        // var headers
        for var_header in &self.var_headers {
            let vh_bytes = unsafe {
                std::slice::from_raw_parts(
                    var_header as *const _ as *const u8,
                    std::mem::size_of::<VarHeader>(),
                )
            };
            buffer.extend_from_slice(vh_bytes);
        }

        // session info length and data
        match &self.session_info {
            Some(info) => {
                buffer.write_u64::<LittleEndian>(info.len() as u64).ok()?;
                buffer.extend_from_slice(info.as_bytes());
            }
            None => {
                buffer.write_u64::<LittleEndian>(0).ok()?;
            }
        }

        // Write raw data length and data
        buffer
            .write_u64::<LittleEndian>(self.raw_data.len() as u64)
            .ok()?;
        buffer.extend_from_slice(&self.raw_data);

        Some(buffer)
    }

    pub fn deserialize(bytes: &[u8]) -> io::Result<Self> {
        let mut cursor = Cursor::new(bytes);

        // header
        let mut header_bytes = [0u8; Header::SIZE];
        cursor.read_exact(&mut header_bytes)?;
        let header: Header = unsafe { std::ptr::read(header_bytes.as_ptr() as *const Header) };

        // var headers
        let var_header_size = std::mem::size_of::<VarHeader>();
        let mut var_headers = Vec::with_capacity(header.num_vars as usize);
        for _ in 0..header.num_vars {
            let mut vh_bytes = vec![0u8; var_header_size];
            cursor.read_exact(&mut vh_bytes)?;
            let var_header: VarHeader =
                unsafe { std::ptr::read(vh_bytes.as_ptr() as *const VarHeader) };
            var_headers.push(var_header);
        }

        // session info
        let session_info_len = cursor.read_u64::<LittleEndian>()? as usize;
        let session_info: Option<String> = if session_info_len > 0 {
            let mut session_info_bytes = vec![0u8; session_info_len];
            cursor.read_exact(&mut session_info_bytes)?;
            Some(String::from_utf8_lossy(&session_info_bytes).to_string())
        } else {
            None
        };

        // data
        let raw_data_len = cursor.read_u64::<LittleEndian>()? as usize;
        let mut raw_data = vec![0u8; raw_data_len];
        cursor.read_exact(&mut raw_data)?;

        Ok(Self {
            header,
            var_headers,
            session_info,
            raw_data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn pad<const N: usize>(s: &[u8]) -> [u8; N] {
        let mut out = [0u8; N];
        let mut i = 0;
        while i < s.len() && i < N {
            out[i] = s[i];
            i += 1;
        }
        out
    }

    #[test]
    fn test_header_size() {
        // Header: 12 ints (48 bytes) + 4 VarBufs (64 bytes) = 112 bytes
        assert_eq!(Header::SIZE, 112);
    }

    #[test]
    fn test_var_header_size() {
        // VarHeader should be 144 bytes (4+4+4+1+3 + 32 + 64 + 32 = 144)
        assert_eq!(std::mem::size_of::<VarHeader>(), 144);
    }

    #[test]
    fn test_var_buf_size() {
        // VarBuf should be 16 bytes (4+4+8 = 16)
        assert_eq!(std::mem::size_of::<VarBuf>(), 16);
    }

    #[test]
    fn test_header_is_connected() {
        let mut header = Header::default();
        assert!(!header.is_connected());

        header.status = StatusField::Connected as i32;
        assert!(header.is_connected());
    }

    #[test]
    fn test_latest_buf_index() {
        let mut header = Header::default();
        header.num_buf = 3;
        header.var_buf[0].tick_count = 100;
        header.var_buf[1].tick_count = 150;
        header.var_buf[2].tick_count = 120;

        assert_eq!(header.latest_buf_index(), 1);
    }

    #[test]
    fn test_serialize_frame_data() {
        let frame = FrameData {
            header: Header {
                ver: 2,
                status: 1,
                tick_rate: 60,
                session_info_update: 5,
                session_info_len: 100,
                session_info_offset: 1000,
                num_vars: 2,
                var_header_offset: 144,
                num_buf: 3,
                buf_len: 512,
                pad1: [0; 2],
                var_buf: [
                    VarBuf {
                        tick_count: 100,
                        buf_offset: 2000,
                        pad: [0; 2],
                    },
                    VarBuf::default(),
                    VarBuf::default(),
                    VarBuf::default(),
                ],
            },
            var_headers: vec![
                VarHeader {
                    var_type: 1,
                    offset: 10,
                    count: 5,
                    count_as_time: 1,
                    pad: [0; 3],
                    name: pad::<IRSDK_MAX_STRING>(b"TestName"),
                    desc: pad::<IRSDK_MAX_DESC>(b"TestDesc"),
                    unit: pad::<IRSDK_MAX_STRING>(b"TestUnit"),
                },
                VarHeader {
                    var_type: 2,
                    offset: 20,
                    count: 10,
                    count_as_time: 0,
                    pad: [0; 3],
                    name: pad::<IRSDK_MAX_STRING>(b"TestName2"),
                    desc: pad::<IRSDK_MAX_DESC>(b"TestDesc2"),
                    unit: pad::<IRSDK_MAX_STRING>(b"TestUnit2"),
                },
            ],
            session_info: Some("SessionInfo:\n  Type: Race\n".to_string()),
            raw_data: vec![1, 2, 3, 4, 5, 6, 7, 8],
        };

        let serialized = frame.serialize();
        assert!(serialized.is_some());
        let serialized = serialized.unwrap();
        let deserialized = FrameData::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.header.ver, frame.header.ver);
        assert_eq!(deserialized.header.status, frame.header.status);
        assert_eq!(deserialized.var_headers.len(), frame.var_headers.len());
        assert_eq!(deserialized.session_info, frame.session_info);
        assert_eq!(deserialized.raw_data, frame.raw_data);
    }

    #[test]
    fn test_serialize_frame_data_no_session_info() {
        let frame = FrameData {
            header: Header {
                ver: 2,
                status: 1,
                tick_rate: 60,
                session_info_update: 5,
                session_info_len: 100,
                session_info_offset: 1000,
                num_vars: 2,
                var_header_offset: 144,
                num_buf: 3,
                buf_len: 512,
                pad1: [0; 2],
                var_buf: [
                    VarBuf {
                        tick_count: 100,
                        buf_offset: 2000,
                        pad: [0; 2],
                    },
                    VarBuf::default(),
                    VarBuf::default(),
                    VarBuf::default(),
                ],
            },
            var_headers: vec![
                VarHeader {
                    var_type: 1,
                    offset: 10,
                    count: 5,
                    count_as_time: 1,
                    pad: [0; 3],
                    name: pad::<IRSDK_MAX_STRING>(b"TestName"),
                    desc: pad::<IRSDK_MAX_DESC>(b"TestDesc"),
                    unit: pad::<IRSDK_MAX_STRING>(b"TestUnit"),
                },
                VarHeader {
                    var_type: 2,
                    offset: 20,
                    count: 10,
                    count_as_time: 0,
                    pad: [0; 3],
                    name: pad::<IRSDK_MAX_STRING>(b"TestName2"),
                    desc: pad::<IRSDK_MAX_DESC>(b"TestDesc2"),
                    unit: pad::<IRSDK_MAX_STRING>(b"TestUnit2"),
                },
            ],
            session_info: None,
            raw_data: vec![1, 2, 3, 4, 5, 6, 7, 8],
        };

        let serialized = frame.serialize();
        assert!(serialized.is_some());
        let serialized = serialized.unwrap();
        let deserialized = FrameData::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.header.ver, frame.header.ver);
        assert_eq!(deserialized.header.status, frame.header.status);
        assert_eq!(deserialized.var_headers.len(), frame.var_headers.len());
        assert_eq!(deserialized.session_info, frame.session_info);
        assert_eq!(deserialized.raw_data, frame.raw_data);
    }
}
