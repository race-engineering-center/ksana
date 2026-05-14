use crate::sims::ac::data::GraphicsPage as AcGraphicsPage;
use crate::sims::ac::data::PhysicsPage as AcPhysicsPage;
use crate::sims::ac::data::StaticPage as AcStaticPage;

pub const CURRENT_PAYLOAD_VERSION: i32 = 2;

pub type PhysicsPage = AcPhysicsPage<1024>; // padded with some headroom, real sizeof in AC is 568 bytes, ACC 800 bytes
pub type GraphicsPage = AcGraphicsPage<2040>; // 8 bytes for packet_id and status
pub type StaticPage = AcStaticPage<2048>; // padded with some headroom, real sizeof in AC is 1044, ACC 1336

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_sizes() {
        assert_eq!(size_of::<PhysicsPage>(), 1024);
        assert_eq!(size_of::<GraphicsPage>(), 2048);
        assert_eq!(size_of::<StaticPage>(), 2048);
    }
}
