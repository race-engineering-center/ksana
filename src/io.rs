// Format:
// - Header (72 bytes):
//   - Magic: "RECROCKS" (8 bytes)
//   - File version: i32 little-endian  (outer container format)
//   - FPS: i32 little-endian
//   - Sim ID: [u8; 4] (4 bytes)
//   - Payload version: i32 little-endian  (sim-specific frame format; added in file v2)
//   - Padding: 48 bytes (reserved for future use)
// - Frames (repeated until EOF):
//   - Header length (at least 12 bytes for header, compressed and raw length): i32
//   - Compressed length: u32 little-endian
//   - Raw length: u32 little-endian
//   - The rest of the header can be reserved for future use
//   - Compressed data: [u8; compressed_length]

use crate::SimInfo;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use std::io::{self, ErrorKind, Read, Seek, SeekFrom, Write};
use thiserror::Error;

const MAGIC: &[u8; 8] = b"RECROCKS";
const PADDING_SIZE: usize = 48; // 72 - 8 (magic) - 4 (version) - 4 (fps) - 4 (id) - 4 (payload_version)
const CURRENT_VERSION: i32 = 2;
const FRAME_HEADER_SIZE: i32 = 12; // header size + compressed len raw len

#[derive(Error, Debug)]
pub enum IOError {
    #[error("Unsupported file version: {0}")]
    UnsupportedVersion(i32),

    #[error("Invalid header size: {0}")]
    InvalidHeaderSize(i32),

    #[error("Invalid file format: expected RECROCKS header")]
    InvalidMagic,

    #[error("Failed to decompress data: file may be corrupted")]
    DecompressionFailed,

    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

pub struct Saver<W: Write> {
    writer: W,
}

impl<W: Write> Saver<W> {
    pub fn new(mut writer: W, fps: i32, info: SimInfo) -> Result<Self, IOError> {
        writer.write_all(MAGIC)?;
        writer.write_i32::<LittleEndian>(CURRENT_VERSION)?;
        writer.write_i32::<LittleEndian>(fps)?;
        writer.write_all(&info.id)?;
        writer.write_i32::<LittleEndian>(info.payload_version)?;

        let padding = [0u8; PADDING_SIZE];
        writer.write_all(&padding)?;

        Ok(Self { writer })
    }

    pub fn save(&mut self, data: &[u8]) -> Result<(), IOError> {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data)?;
        let compressed = encoder.finish()?;

        let compressed_len = compressed.len() as u32;
        let raw_len = data.len() as u32;

        self.writer.write_i32::<LittleEndian>(FRAME_HEADER_SIZE)?;
        self.writer.write_u32::<LittleEndian>(compressed_len)?;
        self.writer.write_u32::<LittleEndian>(raw_len)?;
        self.writer.write_all(&compressed)?;

        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), IOError> {
        self.writer.flush()?;
        Ok(())
    }
}

pub struct Loader<R: Read + Seek> {
    reader: R,
    version: i32,
    payload_version: i32,
    fps: i32,
    id: [u8; 4],
}

impl<R: Read + Seek> Loader<R> {
    pub fn new(mut reader: R) -> Result<Self, IOError> {
        let mut magic = [0u8; 8];
        reader.read_exact(&mut magic)?;
        if &magic != MAGIC {
            return Err(IOError::InvalidMagic);
        }

        let version = reader.read_i32::<LittleEndian>()?;
        if version > CURRENT_VERSION {
            return Err(IOError::UnsupportedVersion(version));
        }

        let fps = reader.read_i32::<LittleEndian>()?;

        let mut id = [0u8; 4];
        reader.read_exact(&mut id)?;

        let payload_version = if version >= 2 {
            let pv = reader.read_i32::<LittleEndian>()?;
            let mut padding = [0u8; PADDING_SIZE];
            reader.read_exact(&mut padding)?;
            pv
        } else {
            let mut padding = [0u8; PADDING_SIZE + 4]; // v1 had 52 bytes of padding
            reader.read_exact(&mut padding)?;
            1
        };

        Ok(Self {
            reader,
            version,
            payload_version,
            fps,
            id,
        })
    }

    pub fn version(&self) -> i32 {
        self.version
    }

    pub fn payload_version(&self) -> i32 {
        self.payload_version
    }

    pub fn fps(&self) -> i32 {
        self.fps
    }

    pub fn id(&self) -> [u8; 4] {
        self.id
    }

    pub fn load(&mut self) -> Result<Option<Vec<u8>>, IOError> {
        let size = self.read_header()?;
        let (compressed_len, raw_len) = match size {
            Some((c, r)) => (c, r),
            None => return Ok(None),
        };

        let mut compressed = vec![0u8; compressed_len];
        self.reader.read_exact(&mut compressed)?;

        let mut decoder = ZlibDecoder::new(&compressed[..]);
        let mut decompressed = Vec::with_capacity(raw_len);
        decoder
            .read_to_end(&mut decompressed)
            .map_err(|_| IOError::DecompressionFailed)?;

        Ok(Some(decompressed))
    }

    pub fn seek(&mut self) -> Result<Option<()>, IOError> {
        let size = self.read_header()?;
        let (compressed_len, _) = match size {
            Some((c, r)) => (c, r),
            None => return Ok(None),
        };

        self.reader.seek(SeekFrom::Current(compressed_len as i64))?;

        Ok(Some(()))
    }

    fn read_header(&mut self) -> Result<Option<(usize, usize)>, IOError> {
        let header_size = match self.reader.read_i32::<LittleEndian>() {
            Ok(size) => size,
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        if header_size < 12 {
            return Err(IOError::InvalidHeaderSize(header_size));
        }

        let compressed_len = match self.reader.read_u32::<LittleEndian>() {
            Ok(len) => len as usize,
            Err(e) => return Err(e.into()),
        };

        let raw_len = self.reader.read_u32::<LittleEndian>()? as usize;

        // Skip any extra header bytes if present
        if self.version() >= 2 {
            let extra_header_bytes = header_size - 12;
            if extra_header_bytes > 0 {
                self.reader
                    .seek(SeekFrom::Current(extra_header_bytes as i64))?;
            }
        }

        Ok(Some((compressed_len, raw_len)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_single_frame() {
        let mut buffer = Vec::new();

        // Write
        {
            let mut saver = Saver::new(
                &mut buffer,
                30,
                SimInfo {
                    id: *b"irac",
                    payload_version: 2,
                },
            )
            .unwrap();
            saver.save(b"hello world").unwrap();
            saver.flush().unwrap();
        }

        // Read
        {
            let mut loader = Loader::new(Cursor::new(&buffer)).unwrap();
            assert_eq!(loader.fps(), 30);
            assert_eq!(&loader.id(), b"irac");

            let frame = loader.load().unwrap();
            assert_eq!(frame, Some(b"hello world".to_vec()));

            // EOF
            assert_eq!(loader.load().unwrap(), None);
        }
    }

    #[test]
    fn test_multiple_frames() {
        let mut buffer = Vec::new();
        let frames: Vec<Vec<u8>> = vec![
            vec![1, 2, 3, 4],
            vec![5, 6, 7, 8, 9, 10],
            vec![0; 1000], // Larger frame to test compression
        ];

        // Write
        {
            let mut saver = Saver::new(
                &mut buffer,
                60,
                SimInfo {
                    id: *b"acsa",
                    payload_version: 2,
                },
            )
            .unwrap();
            for frame in &frames {
                saver.save(frame).unwrap();
            }
            saver.flush().unwrap();
        }

        // Read
        {
            let mut loader = Loader::new(Cursor::new(&buffer)).unwrap();
            assert_eq!(loader.version(), CURRENT_VERSION);
            assert_eq!(loader.fps(), 60);
            assert_eq!(&loader.id(), b"acsa");

            for expected in &frames {
                let frame = loader.load().unwrap();
                assert_eq!(frame.as_ref(), Some(expected));
            }

            // EOF
            assert_eq!(loader.load().unwrap(), None);
        }
    }

    #[test]
    fn test_invalid_magic() {
        let buffer = b"BADMAGIC";
        let result = Loader::new(Cursor::new(buffer));
        assert!(matches!(result, Err(IOError::InvalidMagic)));
    }

    #[test]
    fn test_header_size() {
        let mut buffer = Vec::new();
        let mut saver = Saver::new(
            &mut buffer,
            5,
            SimInfo {
                id: *b"test",
                payload_version: 2,
            },
        )
        .unwrap();
        saver.flush().unwrap();

        // Header should be exactly 72 bytes:
        // - 8 magic
        // - 4 file version
        // - 4 fps
        // - 4 id
        // - 4 payload version
        // - 48 padding
        assert_eq!(buffer.len(), 72);
    }

    #[test]
    fn test_read_payload_version() {
        let mut buffer = Vec::new();
        Saver::new(
            &mut buffer,
            10,
            SimInfo {
                id: *b"irac",
                payload_version: 7,
            },
        )
        .unwrap();

        let loader = Loader::new(Cursor::new(&buffer)).unwrap();
        assert_eq!(loader.fps(), 10);
        assert_eq!(&loader.id(), b"irac");
        assert_eq!(loader.payload_version(), 7);
    }

    #[test]
    fn test_v1_payload_version_defaults_to_1() {
        // Construct a v1 file header manually: magic + version(1) + fps + id + 52 bytes padding.
        let mut buffer = Vec::new();
        buffer.extend_from_slice(MAGIC);
        buffer.extend_from_slice(&1i32.to_le_bytes()); // file version 1
        buffer.extend_from_slice(&5i32.to_le_bytes()); // fps
        buffer.extend_from_slice(b"acsa"); // id
        buffer.extend_from_slice(&[0u8; 52]); // v1 padding (no payload_version field)

        let loader = Loader::new(Cursor::new(&buffer)).unwrap();
        assert_eq!(loader.version(), 1);
        assert_eq!(loader.payload_version(), 1);
    }

    #[test]
    fn test_unsupported_version_rejected() {
        let mut buffer = Vec::new();
        buffer.extend_from_slice(MAGIC);
        buffer.extend_from_slice(&42i32.to_le_bytes());

        let result = Loader::new(Cursor::new(&buffer));
        assert!(matches!(result, Err(IOError::UnsupportedVersion(_))));
    }
}
