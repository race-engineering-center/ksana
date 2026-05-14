//! Generic reusable data structures underlying Assetto Corsa, Assetto Corsa Competizione
//! and Assetto Corsa Evo (the latter uses different page sizes but the same three-page structure).
//! Not intended for direct use by external code.

use std::io;

pub const AC_OFF: i32 = 0;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicsPage<const PADDING: usize> {
    pub content: [u8; PADDING],
}

impl<const PADDING: usize> Default for PhysicsPage<PADDING> {
    fn default() -> Self {
        Self {
            content: [0; PADDING],
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphicsPage<const PADDING: usize> {
    pub packet_id: i32,
    pub status: i32,
    pub content: [u8; PADDING],
}

impl<const PADDING: usize> Default for GraphicsPage<PADDING> {
    fn default() -> Self {
        Self {
            packet_id: 0,
            status: 0,
            content: [0; PADDING],
        }
    }
}

pub const GRAPHICS_STATUS_OFFSET: usize = std::mem::offset_of!(GraphicsPage<0>, status);

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaticPage<const PADDING: usize> {
    pub content: [u8; PADDING],
}

impl<const PADDING: usize> Default for StaticPage<PADDING> {
    fn default() -> Self {
        Self {
            content: [0; PADDING],
        }
    }
}

// All sim frame payloads begin with a 16-byte frame header: 1 byte type + 15 bytes reserved.
// This is the standard across all sims and allows future extension without a file version bump.
const FRAME_TYPE_WITH_STATICS: u8 = 0x01;
const FRAME_TYPE_NO_STATICS: u8 = 0x02;
const FRAME_HEADER_SIZE: usize = 16;

pub trait SimPage: Default + Copy {}

impl<const PADDING: usize> SimPage for PhysicsPage<PADDING> {}
impl<const PADDING: usize> SimPage for GraphicsPage<PADDING> {}
impl<const PADDING: usize> SimPage for StaticPage<PADDING> {}

pub trait GraphicsLike: SimPage {
    fn status(&self) -> i32;
}
pub trait PhysicsLike: SimPage {}
pub trait StaticLike: SimPage + PartialEq {}

impl<const PADDING: usize> GraphicsLike for GraphicsPage<PADDING> {
    fn status(&self) -> i32 {
        self.status
    }
}
impl<const PADDING: usize> PhysicsLike for PhysicsPage<PADDING> {}
impl<const PADDING: usize> StaticLike for StaticPage<PADDING> {}

pub struct FrameData<G: GraphicsLike, P: PhysicsLike, S: StaticLike> {
    pub graphics: G,
    pub physics: P,
    pub statics: Option<S>,
}

impl<G: GraphicsLike, P: PhysicsLike, S: StaticLike> Default for FrameData<G, P, S> {
    fn default() -> Self {
        Self {
            graphics: G::default(),
            physics: P::default(),
            statics: None,
        }
    }
}

impl<G: GraphicsLike, P: PhysicsLike, S: StaticLike> FrameData<G, P, S> {
    pub const fn graphics_size() -> usize {
        size_of::<G>()
    }
    pub const fn physics_size() -> usize {
        size_of::<P>()
    }
    pub const fn static_size() -> usize {
        size_of::<S>()
    }

    pub fn serialize(&self) -> Vec<u8> {
        let total_size = if self.statics.is_some() {
            FRAME_HEADER_SIZE + Self::graphics_size() + Self::physics_size() + Self::static_size()
        } else {
            FRAME_HEADER_SIZE + Self::graphics_size() + Self::physics_size()
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
                &self.graphics as *const G as *const u8,
                Self::graphics_size(),
            )
        };
        let graphics_offset = FRAME_HEADER_SIZE;
        buffer[graphics_offset..graphics_offset + graphics_bytes.len()]
            .copy_from_slice(graphics_bytes);

        // physics
        let physics_bytes = unsafe {
            std::slice::from_raw_parts(&self.physics as *const P as *const u8, Self::physics_size())
        };
        let physics_offset = graphics_offset + Self::graphics_size();
        buffer[physics_offset..physics_offset + physics_bytes.len()].copy_from_slice(physics_bytes);

        // statics
        if let Some(statics) = &self.statics {
            let statics_bytes = unsafe {
                std::slice::from_raw_parts(statics as *const S as *const u8, Self::static_size())
            };
            let statics_offset = physics_offset + Self::physics_size();
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
            let frame_size_no_statics = Self::graphics_size() + Self::physics_size();
            let frame_size = frame_size_no_statics + Self::static_size();
            let has_statics = if bytes.len() == frame_size_no_statics {
                false
            } else if bytes.len() == frame_size {
                true
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Buffer size does not match expected sizes for AC frame data",
                ));
            };
            (has_statics, 0)
        };

        let min_size = data_offset + Self::graphics_size() + Self::physics_size();
        if bytes.len() < min_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Buffer too small for Assetto Corsa frame data",
            ));
        }

        let mut result = Self::default();

        // graphics
        let graphics_offset = data_offset;
        let graphics_size = Self::graphics_size();
        unsafe {
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr().add(graphics_offset),
                &mut result.graphics as *mut G as *mut u8,
                graphics_size,
            );
        }

        // physics
        let physics_offset = data_offset + Self::graphics_size();
        let physics_size = Self::physics_size();
        unsafe {
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr().add(physics_offset),
                &mut result.physics as *mut P as *mut u8,
                physics_size,
            );
        }

        // statics
        if has_statics {
            let statics_offset = physics_offset + Self::physics_size();
            let statics_size = Self::static_size();
            unsafe {
                let mut statics = S::default();
                std::ptr::copy_nonoverlapping(
                    bytes.as_ptr().add(statics_offset),
                    &mut statics as *mut S as *mut u8,
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

    type G = GraphicsPage<2040>;
    type P = PhysicsPage<1024>;
    type S = StaticPage<1988>;
    type Frame = FrameData<G, P, S>;

    #[test]
    fn test_graphics_status_offset() {
        // needed for writing the AC_OFF status upon exit
        assert_eq!(GRAPHICS_STATUS_OFFSET, 4);
    }

    #[test]
    fn test_default_frame_data_is_zero() {
        let frame = Frame::default();
        assert_eq!(frame.graphics.packet_id, 0);
        assert_eq!(frame.graphics.status, 0);
        assert!(frame.graphics.content.iter().all(|&b| b == 0));
        assert!(frame.physics.content.iter().all(|&b| b == 0));
        assert!(frame.statics.is_none());
    }

    #[test]
    fn test_serialize_frame_data_no_statics() {
        let mut frame = Frame::default();
        frame.graphics.packet_id = 42;
        frame.graphics.status = 3;
        frame.graphics.content[0] = 0xAB;
        frame.physics.content[0] = 0xCD;

        let deserialized = Frame::deserialize(&frame.serialize(), 2).unwrap();

        assert_eq!(deserialized.graphics.packet_id, 42);
        assert_eq!(deserialized.graphics.status, 3);
        assert_eq!(deserialized.graphics.content[0], 0xAB);
        assert_eq!(deserialized.physics.content[0], 0xCD);
        assert!(deserialized.statics.is_none());
    }

    #[test]
    fn test_deserialize_v1_backward_compat() {
        // Simulate a v1 recording: raw struct bytes with no frame header,
        // graphics at offset 0, physics immediately after.
        let mut frame = Frame::default();
        frame.graphics.packet_id = 7;
        frame.graphics.content[0] = 0x11;
        frame.physics.content[0] = 0x22;

        let v1_size = Frame::graphics_size() + Frame::physics_size();
        let mut bytes = vec![0u8; v1_size];
        unsafe {
            std::ptr::copy_nonoverlapping(
                &frame.graphics as *const G as *const u8,
                bytes.as_mut_ptr(),
                Frame::graphics_size(),
            );
            std::ptr::copy_nonoverlapping(
                &frame.physics as *const P as *const u8,
                bytes.as_mut_ptr().add(Frame::graphics_size()),
                Frame::physics_size(),
            );
        }

        let deserialized = Frame::deserialize(&bytes, 1).unwrap();

        assert_eq!(deserialized.graphics.packet_id, 7);
        assert_eq!(deserialized.graphics.content[0], 0x11);
        assert_eq!(deserialized.physics.content[0], 0x22);
        assert!(deserialized.statics.is_none());
    }

    #[test]
    fn test_deserialize_v1_backward_compat_with_statics() {
        let v1_size = Frame::graphics_size() + Frame::physics_size() + Frame::static_size();
        let mut frame = Frame::default();
        frame.graphics.packet_id = 9;
        frame.graphics.content[0] = 0x33;
        frame.physics.content[0] = 0x44;
        let mut statics = S::default();
        statics.content[0] = 0x55;

        let mut bytes = vec![0u8; v1_size];
        unsafe {
            std::ptr::copy_nonoverlapping(
                &frame.graphics as *const G as *const u8,
                bytes.as_mut_ptr(),
                Frame::graphics_size(),
            );
            std::ptr::copy_nonoverlapping(
                &frame.physics as *const P as *const u8,
                bytes.as_mut_ptr().add(Frame::graphics_size()),
                Frame::physics_size(),
            );
            std::ptr::copy_nonoverlapping(
                &statics as *const S as *const u8,
                bytes
                    .as_mut_ptr()
                    .add(Frame::graphics_size() + Frame::physics_size()),
                Frame::static_size(),
            );
        }

        let deserialized = Frame::deserialize(&bytes, 1).unwrap();

        assert_eq!(deserialized.graphics.packet_id, 9);
        assert_eq!(deserialized.graphics.content[0], 0x33);
        assert_eq!(deserialized.physics.content[0], 0x44);
        assert_eq!(deserialized.statics.unwrap().content[0], 0x55);
    }

    #[test]
    fn test_serialize_frame_data_with_statics() {
        let mut frame = Frame::default();
        frame.graphics.status = 1;
        frame.graphics.packet_id = 123;

        let graphics_data = b"Graphics telemetry data from AC";
        frame.graphics.content[..graphics_data.len()].copy_from_slice(graphics_data);

        let physics_data = b"Physics simulation data";
        frame.physics.content[..physics_data.len()].copy_from_slice(physics_data);

        let mut statics = S::default();
        let statics_data = b"Car: Ferrari 488 GT3, Track: Fancy Test Track";
        statics.content[..statics_data.len()].copy_from_slice(statics_data);
        frame.statics = Some(statics);

        let deserialized = Frame::deserialize(&frame.serialize(), 2).unwrap();

        assert_eq!(deserialized.graphics.packet_id, 123);
        assert_eq!(deserialized.graphics.status, 1);
        assert_eq!(
            &deserialized.graphics.content[..graphics_data.len()],
            graphics_data
        );
        assert_eq!(
            &deserialized.physics.content[..physics_data.len()],
            physics_data
        );
        assert_eq!(
            &deserialized.statics.unwrap().content[..statics_data.len()],
            statics_data
        );
    }
}
