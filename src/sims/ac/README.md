# Assetto Corsa generic shared memory utils

Shared implementation for sims that expose telemetry through the Assetto Corsa
shared-memory protocol. The protocol uses three named memory-mapped files:

- graphics page
- physics page
- statics page

Graphics page contains a packet id (unused) and a status used to determine if AC
sim is running. Statics page is optional in the frame, we skip recording it if
it didn't change. Normally we only expect it to change when the session is
changed.

The rest of the page is a `u8` padding with a generic size, allowing a concrete
implementation to just specialize the size of the padding.

## Components

### Data

Contained in a `data.rs` file, provides the structures for all three shared
memory pages generic on the padding as well as supporting traits.

`GraphicsLike` trait exposes a method to get a sim status needed by the
connection logic.

`StaticLike` trait is needed to be able to compare the current static page with
the previous one and skip serializing it if it didn't change.

This module also implements a frame struct which is generic on the 3 pages along
with the generic serialization/deserialization logic.

### shmio

Shared memory reader/writer layer. `SharedMemoryReader` and `SharedMemoryWriter`
are thin generic wrappers over `crate::shm` that bind the three page names to
the three page types, so the rest of the module never deals with raw mappings or
sizes. Both classes are generic on the Graphics, Physics and Static pages,
enabling reading of shared memory segments of any size.

The reader exposes one method per page. Each is a raw `ptr::read` of the mapped
region.

The writer's `update` takes a serialized frame, deserializes it, and copies the
pages into their mappings. Statics is written only if present in the frame.
`stop` writes `AC_OFF` into the graphics status field so any attached reader
sees the sim as gone, then drops the mappings.

### Connector

Implements `crate::Connector` on top of the `SharedMemoryReader`, also generic
on the three pages.

`connect` opens the three pages and reads the graphics status. If it is `AC_OFF`
the game isn't running, so the connection is rejected and retried later.

`update` reads all three pages and serializes a frame. The static page is only
included if it differs from the one seen on the previous tick, comparing against
a cached copy.

### Player

Implements `crate::Player` on top of the `SharedMemoryWriter`, also generic on
the three pages. The replay side: it forwards `update` and `stop` to
the writer. The writer is constructed by the concrete sim and handed in, so
this module owns no page creation or failure handling.
