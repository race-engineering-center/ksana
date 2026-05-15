use crate::sims::ac::connector::Connector as AcConnector;

use super::data::{CURRENT_PAYLOAD_VERSION, GraphicsPage, PhysicsPage, StaticPage};
use super::shm::{AC_GRAPHICS_SHM, AC_PHYSICS_SHM, AC_STATIC_SHM};

pub type AssettoCorsaConnector = AcConnector<GraphicsPage, PhysicsPage, StaticPage>;

impl Default for AssettoCorsaConnector {
    fn default() -> Self {
        Self::new(
            AC_GRAPHICS_SHM,
            AC_PHYSICS_SHM,
            AC_STATIC_SHM,
            *b"acsa",
            CURRENT_PAYLOAD_VERSION,
        )
    }
}
