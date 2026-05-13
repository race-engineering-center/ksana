use std::io;

pub const CURRENT_PAYLOAD_VERSION: i32 = 2;

pub const SHM_SIZE: usize = 2048;

pub const AC_OFF: i32 = 0;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicsPage {
    pub content: [u8; 1024], // padded with some headroom, real sizeof in AC is 568 bytes, ACC 800 bytes
}

const PHYSICS_PADDED_SIZE: usize = size_of::<PhysicsPage>();

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphicsPage {
    pub packet_id: i32,
    pub status: i32,
    pub content: [u8; 2040], // padded with some headroom, real sizeof in AC is 468, ACC is 1892
}

const GRAPHICS_PADDED_SIZE: usize = size_of::<GraphicsPage>();

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaticPage {
    pub sm_version: [u16; 15],
    pub ac_version: [u16; 15],
    pub content: [u8; 1988], // padded with some headroom, real sizeof in AC is 1044, ACC 1336
}

const STATIC_PADDED_SIZE: usize = size_of::<StaticPage>();

impl Default for PhysicsPage {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

impl Default for GraphicsPage {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

impl Default for StaticPage {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

// All sim frame payloads begin with a 16-byte frame header: 1 byte type + 15 bytes reserved.
// This is the standard across all sims and allows future extension without a file version bump.
const FRAME_TYPE_WITH_STATICS: u8 = 0x01;
const FRAME_TYPE_NO_STATICS: u8 = 0x02;
const FRAME_HEADER_SIZE: usize = 16;

// v1 file doesn't contain a frame header, and statics presence is inferred from buffer size
mod v1 {
    use super::{GRAPHICS_PADDED_SIZE, PHYSICS_PADDED_SIZE};
    pub const FRAME_SIZE_NO_STATICS: usize = GRAPHICS_PADDED_SIZE + PHYSICS_PADDED_SIZE;
}

// v2 file contains a frame header with explicit frame type, allowing statics to be optional without relying on buffer size
mod v2 {
    use super::{FRAME_HEADER_SIZE, GRAPHICS_PADDED_SIZE, PHYSICS_PADDED_SIZE, STATIC_PADDED_SIZE};
    pub const FRAME_SIZE: usize =
        FRAME_HEADER_SIZE + GRAPHICS_PADDED_SIZE + PHYSICS_PADDED_SIZE + STATIC_PADDED_SIZE;
    pub const FRAME_SIZE_NO_STATICS: usize =
        FRAME_HEADER_SIZE + GRAPHICS_PADDED_SIZE + PHYSICS_PADDED_SIZE;
}

#[derive(Debug, Clone, Default)]
pub struct FrameData {
    pub graphics: GraphicsPage,
    pub physics: PhysicsPage,
    pub statics: Option<StaticPage>,
}

impl FrameData {
    pub fn serialize(&self) -> Vec<u8> {
        let total_size = if self.statics.is_some() {
            v2::FRAME_SIZE
        } else {
            v2::FRAME_SIZE_NO_STATICS
        };
        let mut buffer = vec![0u8; total_size];

        // frame header: type byte + reserved padding
        buffer[0] = if self.statics.is_some() {
            FRAME_TYPE_WITH_STATICS
        } else {
            FRAME_TYPE_NO_STATICS
        };

        // graphics
        let graphics_bytes = unsafe {
            std::slice::from_raw_parts(
                &self.graphics as *const GraphicsPage as *const u8,
                std::mem::size_of::<GraphicsPage>(),
            )
        };
        let graphics_offset = FRAME_HEADER_SIZE;
        buffer[graphics_offset..graphics_offset + graphics_bytes.len()]
            .copy_from_slice(graphics_bytes);

        // physics
        let physics_bytes = unsafe {
            std::slice::from_raw_parts(
                &self.physics as *const PhysicsPage as *const u8,
                std::mem::size_of::<PhysicsPage>(),
            )
        };
        let physics_offset = FRAME_HEADER_SIZE + GRAPHICS_PADDED_SIZE;
        buffer[physics_offset..physics_offset + physics_bytes.len()].copy_from_slice(physics_bytes);

        // statics
        if let Some(statics) = &self.statics {
            let statics_bytes = unsafe {
                std::slice::from_raw_parts(
                    statics as *const StaticPage as *const u8,
                    std::mem::size_of::<StaticPage>(),
                )
            };
            let statics_offset = FRAME_HEADER_SIZE + GRAPHICS_PADDED_SIZE + PHYSICS_PADDED_SIZE;
            buffer[statics_offset..statics_offset + statics_bytes.len()]
                .copy_from_slice(statics_bytes);
        }

        buffer
    }

    pub fn deserialize(bytes: &[u8], payload_version: i32) -> io::Result<Self> {
        let (has_statics, data_offset) = if payload_version >= 2 {
            if bytes.len() < FRAME_HEADER_SIZE {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Buffer too small for frame header",
                ));
            }
            let frame_type = bytes[0];
            if frame_type != FRAME_TYPE_WITH_STATICS && frame_type != FRAME_TYPE_NO_STATICS {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Unknown AC frame type: {frame_type:#04x}"),
                ));
            }
            (frame_type == FRAME_TYPE_WITH_STATICS, FRAME_HEADER_SIZE)
        } else {
            // v1: no frame header, infer statics from buffer size
            let has_statics = bytes.len() > v1::FRAME_SIZE_NO_STATICS;
            (has_statics, 0)
        };

        let min_size = data_offset + GRAPHICS_PADDED_SIZE + PHYSICS_PADDED_SIZE;
        if bytes.len() < min_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Buffer too small for Assetto Corsa frame data",
            ));
        }

        let mut result = Self::default();

        // graphics
        let graphics_offset = data_offset;
        let graphics_size = std::mem::size_of::<GraphicsPage>();
        unsafe {
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr().add(graphics_offset),
                &mut result.graphics as *mut GraphicsPage as *mut u8,
                graphics_size,
            );
        }

        // physics
        let physics_offset = data_offset + GRAPHICS_PADDED_SIZE;
        let physics_size = std::mem::size_of::<PhysicsPage>();
        unsafe {
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr().add(physics_offset),
                &mut result.physics as *mut PhysicsPage as *mut u8,
                physics_size,
            );
        }

        // statics
        if has_statics {
            let statics_offset = data_offset + GRAPHICS_PADDED_SIZE + PHYSICS_PADDED_SIZE;
            let statics_size = std::mem::size_of::<StaticPage>();
            unsafe {
                let mut statics = StaticPage::default();
                std::ptr::copy_nonoverlapping(
                    bytes.as_ptr().add(statics_offset),
                    &mut statics as *mut StaticPage as *mut u8,
                    statics_size,
                );
                result.statics = Some(statics);
            }
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_sizes() {
        assert_eq!(PHYSICS_PADDED_SIZE, 1024);
        assert_eq!(GRAPHICS_PADDED_SIZE, 2048);
        assert_eq!(STATIC_PADDED_SIZE, 2048);
    }

    #[test]
    fn test_default_frame_data_is_zero() {
        let frame = FrameData::default();

        // Graphics should be zero
        assert_eq!(frame.graphics.packet_id, 0);
        assert_eq!(frame.graphics.status, 0);
        assert!(frame.graphics.content.iter().all(|&b| b == 0));

        // Physics should be zero
        assert!(frame.physics.content.iter().all(|&b| b == 0));

        // Statics should be None
        assert!(frame.statics.is_none());
    }

    #[test]
    fn test_serialize_frame_data_no_statics() {
        let mut frame = FrameData::default();
        frame.graphics.packet_id = 42;
        frame.graphics.status = 3;
        frame.graphics.content[0] = 0xAB;
        frame.physics.content[0] = 0xCD;

        let serialized = frame.serialize();
        let deserialized = FrameData::deserialize(&serialized, 2).unwrap();

        assert_eq!(deserialized.graphics.packet_id, frame.graphics.packet_id);
        assert_eq!(deserialized.graphics.status, frame.graphics.status);
        assert_eq!(deserialized.graphics.content[0], 0xAB);
        assert_eq!(deserialized.physics.content[0], 0xCD);
        assert!(deserialized.statics.is_none());
    }

    #[test]
    fn test_deserialize_v1_backward_compat() {
        // Simulate a v1 recording: raw struct bytes with no frame header,
        // graphics at offset 0, physics immediately after.
        let mut frame = FrameData::default();
        frame.graphics.packet_id = 7;
        frame.graphics.content[0] = 0x11;
        frame.physics.content[0] = 0x22;

        let mut bytes = vec![0u8; v1::FRAME_SIZE_NO_STATICS];
        unsafe {
            std::ptr::copy_nonoverlapping(
                &frame.graphics as *const GraphicsPage as *const u8,
                bytes.as_mut_ptr(),
                size_of::<GraphicsPage>(),
            );
            std::ptr::copy_nonoverlapping(
                &frame.physics as *const PhysicsPage as *const u8,
                bytes.as_mut_ptr().add(GRAPHICS_PADDED_SIZE),
                size_of::<PhysicsPage>(),
            );
        }

        let deserialized = FrameData::deserialize(&bytes, 1).unwrap();

        assert_eq!(deserialized.graphics.packet_id, 7);
        assert_eq!(deserialized.graphics.content[0], 0x11);
        assert_eq!(deserialized.physics.content[0], 0x22);
        assert!(deserialized.statics.is_none());
    }

    #[test]
    fn test_deserialize_v1_backward_compat_with_statics() {
        const V1_FRAME_SIZE: usize =
            GRAPHICS_PADDED_SIZE + PHYSICS_PADDED_SIZE + STATIC_PADDED_SIZE;
        // Simulate a v1 recording with statics: graphics + physics + statics, no frame header.
        let mut frame = FrameData::default();
        frame.graphics.packet_id = 9;
        frame.graphics.content[0] = 0x33;
        frame.physics.content[0] = 0x44;
        let mut statics = StaticPage::default();
        statics.content[0] = 0x55;

        let mut bytes = vec![0u8; V1_FRAME_SIZE];
        unsafe {
            std::ptr::copy_nonoverlapping(
                &frame.graphics as *const GraphicsPage as *const u8,
                bytes.as_mut_ptr(),
                size_of::<GraphicsPage>(),
            );
            std::ptr::copy_nonoverlapping(
                &frame.physics as *const PhysicsPage as *const u8,
                bytes.as_mut_ptr().add(GRAPHICS_PADDED_SIZE),
                size_of::<PhysicsPage>(),
            );
            std::ptr::copy_nonoverlapping(
                &statics as *const StaticPage as *const u8,
                bytes
                    .as_mut_ptr()
                    .add(GRAPHICS_PADDED_SIZE + PHYSICS_PADDED_SIZE),
                size_of::<StaticPage>(),
            );
        }

        let deserialized = FrameData::deserialize(&bytes, 1).unwrap();

        assert_eq!(deserialized.graphics.packet_id, 9);
        assert_eq!(deserialized.graphics.content[0], 0x33);
        assert_eq!(deserialized.physics.content[0], 0x44);
        assert_eq!(deserialized.statics.unwrap().content[0], 0x55);
    }

    #[test]
    fn test_serialize_frame_data() {
        let mut frame = FrameData::default();
        frame.graphics.status = 1;
        frame.graphics.packet_id = 123;

        // Copy graphics data string
        let graphics_data = b"Graphics telemetry data from AC";
        let copy_len = graphics_data.len();
        frame.graphics.content[..copy_len].copy_from_slice(&graphics_data[..copy_len]);

        // Copy physics data string
        let physics_data = b"Physics simulation data";
        let copy_len = physics_data.len();
        frame.physics.content[..copy_len].copy_from_slice(&physics_data[..copy_len]);

        // Set version values to match byte strings when interpreted as little-endian
        frame.statics = Some(StaticPage::default());
        frame.statics.as_mut().unwrap().sm_version[0] = u16::from_le_bytes(*b"SM");
        frame.statics.as_mut().unwrap().sm_version[1] = u16::from_le_bytes(*b"v1");

        frame.statics.as_mut().unwrap().ac_version[0] = u16::from_le_bytes(*b"AC");
        frame.statics.as_mut().unwrap().ac_version[1] = u16::from_le_bytes(*b"v2");

        let statics_data = b"Car: Ferrari 488 GT3, Track: Fancy Test Track, Player: TestDriver"; // not really, but whatever
        let copy_len = statics_data.len();
        frame.statics.as_mut().unwrap().content[..copy_len]
            .copy_from_slice(&statics_data[..copy_len]);

        let serialized = frame.serialize();
        let deserialized = FrameData::deserialize(&serialized, 2).unwrap();

        // Graphics
        assert_eq!(deserialized.graphics.packet_id, frame.graphics.packet_id);
        assert_eq!(deserialized.graphics.status, frame.graphics.status);
        assert_eq!(
            &deserialized.graphics.content[..graphics_data.len()],
            &frame.graphics.content[..graphics_data.len()]
        );

        // Physics
        assert_eq!(
            &deserialized.physics.content[..physics_data.len()],
            &frame.physics.content[..physics_data.len()]
        );

        // Statics - verify versions can be compared as byte strings
        assert_eq!(
            deserialized.statics.as_ref().unwrap().sm_version[0].to_le_bytes(),
            *b"SM"
        );
        assert_eq!(
            deserialized.statics.as_ref().unwrap().sm_version[1].to_le_bytes(),
            *b"v1"
        );
        assert_eq!(
            deserialized.statics.as_ref().unwrap().ac_version[0].to_le_bytes(),
            *b"AC"
        );
        assert_eq!(
            deserialized.statics.as_ref().unwrap().ac_version[1].to_le_bytes(),
            *b"v2"
        );
        assert_eq!(
            &deserialized.statics.as_ref().unwrap().content[..statics_data.len()],
            &frame.statics.as_ref().unwrap().content[..statics_data.len()]
        );
    }
}
