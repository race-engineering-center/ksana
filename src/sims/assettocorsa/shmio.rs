use crate::sims::ac::shmio::SharedMemoryReader as ShmReader;
use crate::sims::ac::shmio::SharedMemoryWriter as ShmWriter;

use crate::sims::assettocorsa::data::GraphicsPage;
use crate::sims::assettocorsa::data::PhysicsPage;
use crate::sims::assettocorsa::data::StaticPage;

pub type SharedMemoryReader = ShmReader<GraphicsPage, PhysicsPage, StaticPage>;
pub type SharedMemoryWriter = ShmWriter<GraphicsPage, PhysicsPage, StaticPage>;
