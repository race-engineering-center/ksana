use std::io;

pub const AC_GRAPHICS_SHM: &str = "Local\\acpmf_graphics";
pub const AC_PHYSICS_SHM: &str = "Local\\acpmf_physics";
pub const AC_STATIC_SHM: &str = "Local\\acpmf_static";

pub const SHM_SIZE: usize = 2048;

pub const AC_OFF: i32 = 0;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PhysicsPage {
    pub content: [u8; 1024], // padded with some headroom, real sizeof in AC is 568 bytes, ACC 800 bytes
}

const PHYSICS_PADDED_SIZE: usize = size_of::<PhysicsPage>();

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GraphicsPage {
    pub packet_id: i32,
    pub status: i32,
    pub content: [u8; 2040], // padded with some headroom, real sizeof in AC is 468, ACC is 1892
}

const GRAPHICS_PADDED_SIZE: usize = size_of::<GraphicsPage>();

#[repr(C)]
#[derive(Debug, Clone, Copy)]
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

// Frame data and serialization
pub const FRAME_SIZE: usize = GRAPHICS_PADDED_SIZE + PHYSICS_PADDED_SIZE + STATIC_PADDED_SIZE;

#[derive(Debug, Clone, Default)]
pub struct FrameData {
    pub graphics: GraphicsPage,
    pub physics: PhysicsPage,
    pub statics: StaticPage,
}

impl FrameData {
    pub fn serialize(&self) -> Vec<u8> {
        let mut buffer = vec![0u8; FRAME_SIZE];

        // graphics
        let graphics_bytes = unsafe {
            std::slice::from_raw_parts(
                &self.graphics as *const _ as *const u8,
                std::mem::size_of::<GraphicsPage>(),
            )
        };
        buffer[..graphics_bytes.len()].copy_from_slice(graphics_bytes);

        // physics
        let physics_bytes = unsafe {
            std::slice::from_raw_parts(
                &self.physics as *const _ as *const u8,
                std::mem::size_of::<PhysicsPage>(),
            )
        };
        let physics_offset = GRAPHICS_PADDED_SIZE;
        buffer[physics_offset..physics_offset + physics_bytes.len()].copy_from_slice(physics_bytes);

        // statics
        let statics_bytes = unsafe {
            std::slice::from_raw_parts(
                &self.statics as *const _ as *const u8,
                std::mem::size_of::<StaticPage>(),
            )
        };
        let statics_offset = GRAPHICS_PADDED_SIZE + PHYSICS_PADDED_SIZE;
        buffer[statics_offset..statics_offset + statics_bytes.len()].copy_from_slice(statics_bytes);

        buffer
    }

    pub fn deserialize(bytes: &[u8]) -> io::Result<Self> {
        if bytes.len() < FRAME_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Buffer too small for ACC frame data",
            ));
        }

        let mut result = Self::default();

        // graphics
        let graphics_size = std::mem::size_of::<GraphicsPage>();
        unsafe {
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                &mut result.graphics as *mut _ as *mut u8,
                graphics_size,
            );
        }

        // physics
        let physics_offset = GRAPHICS_PADDED_SIZE;
        let physics_size = std::mem::size_of::<PhysicsPage>();
        unsafe {
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr().add(physics_offset),
                &mut result.physics as *mut _ as *mut u8,
                physics_size,
            );
        }

        // statics
        let statics_offset = GRAPHICS_PADDED_SIZE + PHYSICS_PADDED_SIZE;
        let statics_size = std::mem::size_of::<StaticPage>();
        unsafe {
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr().add(statics_offset),
                &mut result.statics as *mut _ as *mut u8,
                statics_size,
            );
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

        // Statics should be zero
        assert!(frame.statics.sm_version.iter().all(|&v| v == 0));
        assert!(frame.statics.ac_version.iter().all(|&v| v == 0));
        assert!(frame.statics.content.iter().all(|&b| b == 0));
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
        frame.statics.sm_version[0] = u16::from_le_bytes(*b"SM");
        frame.statics.sm_version[1] = u16::from_le_bytes(*b"v1");

        frame.statics.ac_version[0] = u16::from_le_bytes(*b"AC");
        frame.statics.ac_version[1] = u16::from_le_bytes(*b"v2");

        let statics_data = b"Car: Ferrari 488 GT3, Track: Fancy Test Track, Player: TestDriver"; // not really, but whatever
        let copy_len = statics_data.len();
        frame.statics.content[..copy_len].copy_from_slice(&statics_data[..copy_len]);

        let serialized = frame.serialize();
        let deserialized = FrameData::deserialize(&serialized).unwrap();

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
        assert_eq!(deserialized.statics.sm_version[0].to_le_bytes(), *b"SM");
        assert_eq!(deserialized.statics.sm_version[1].to_le_bytes(), *b"v1");
        assert_eq!(deserialized.statics.ac_version[0].to_le_bytes(), *b"AC");
        assert_eq!(deserialized.statics.ac_version[1].to_le_bytes(), *b"v2");
        assert_eq!(
            &deserialized.statics.content[..statics_data.len()],
            &frame.statics.content[..statics_data.len()]
        );
    }
}
