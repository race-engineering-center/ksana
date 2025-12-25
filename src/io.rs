// Format:
// - Header (72 bytes):
//   - Magic: "RECROCKS" (8 bytes)
//   - File version i32 little-endian
//   - FPS: i32 little-endian
//   - Sim ID: [u8; 4] (4 bytes)
//   - Padding: 52 bytes (reserved for future use)
// - Frames (repeated until EOF):
//   - Header length (at least 12 bytes for header, compressed and raw length): i32
//   - Compressed length: u32 little-endian
//   - Raw length: u32 little-endian
//   - The rest of the header can be reserved for future use
//   - Compressed data: [u8; compressed_length]

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use std::io::{self, ErrorKind, Read, Write};
use thiserror::Error;

const MAGIC: &[u8; 8] = b"RECROCKS";
const PADDING_SIZE: usize = 52; // 72 - 8 (magic) - 4 (fps) - 4 (id)
const CURRENT_VERSION: i32 = 1;
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
    pub fn new(mut writer: W, fps: i32, id: [u8; 4]) -> Result<Self, IOError> {
        writer.write_all(MAGIC)?;

        // file version
        writer.write_i32::<LittleEndian>(1)?;

        // fps
        writer.write_i32::<LittleEndian>(fps)?;

        // sim ID
        writer.write_all(&id)?;

        let padding = [0u8; PADDING_SIZE];
        writer.write_all(&padding)?;

        Ok(Self { writer })
    }

    pub fn save(&mut self, data: &[u8]) -> Result<(), IOError> {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data)?;
        let compressed = encoder.finish()?;

        self.writer.write_i32::<LittleEndian>(FRAME_HEADER_SIZE)?;
        self.writer
            .write_u32::<LittleEndian>(compressed.len() as u32)?;
        self.writer.write_u32::<LittleEndian>(data.len() as u32)?;
        self.writer.write_all(&compressed)?;

        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), IOError> {
        self.writer.flush()?;
        Ok(())
    }
}

pub struct Loader<R: Read> {
    reader: R,
    version: i32,
    fps: i32,
    id: [u8; 4],
}

impl<R: Read> Loader<R> {
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

        let mut padding = [0u8; PADDING_SIZE];
        reader.read_exact(&mut padding)?;

        Ok(Self {
            version,
            reader,
            fps,
            id,
        })
    }

    pub fn version(&self) -> i32 {
        self.version
    }

    pub fn fps(&self) -> i32 {
        self.fps
    }

    pub fn id(&self) -> [u8; 4] {
        self.id
    }

    pub fn load(&mut self) -> Result<Option<Vec<u8>>, IOError> {
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
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        let raw_len = self.reader.read_u32::<LittleEndian>()? as usize;

        // skip any extra header bytes if present
        // version() check is used here just to silence the unused warning
        if self.version() == CURRENT_VERSION {
            for _ in 0..(header_size - 12) {
                let _ = self.reader.read_u8()?;
            }
        }

        let mut compressed = vec![0u8; compressed_len];
        self.reader.read_exact(&mut compressed)?;

        let mut decoder = ZlibDecoder::new(&compressed[..]);
        let mut decompressed = Vec::with_capacity(raw_len);
        decoder
            .read_to_end(&mut decompressed)
            .map_err(|_| IOError::DecompressionFailed)?;

        Ok(Some(decompressed))
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
            let mut saver = Saver::new(&mut buffer, 30, *b"irac").unwrap();
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
            let mut saver = Saver::new(&mut buffer, 60, *b"acsa").unwrap();
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
        let mut saver = Saver::new(&mut buffer, 5, *b"test").unwrap();
        saver.flush().unwrap();

        // Header should be exactly 72 bytes:
        // - 8 magic
        // - 4 file version
        // - 4 fps
        // - 4 id
        // - padding
        assert_eq!(buffer.len(), 72);
    }
}
