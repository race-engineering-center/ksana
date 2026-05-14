use crate::sims::ac::connector::Connector as AcConnector;

use super::data::CURRENT_PAYLOAD_VERSION;
use super::shm::{AC_GRAPHICS_SHM, AC_PHYSICS_SHM, AC_STATIC_SHM};
use super::shmio::SharedMemoryReader;

pub type AssettoCorsaConnector = AcConnector<SharedMemoryReader>;

impl Default for AssettoCorsaConnector {
    fn default() -> Self {
        Self::new(
            AC_GRAPHICS_SHM.to_string(),
            AC_PHYSICS_SHM.to_string(),
            AC_STATIC_SHM.to_string(),
            *b"acsa",
            CURRENT_PAYLOAD_VERSION,
        )
    }
}
