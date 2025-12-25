use std::ffi::CString;
use std::ptr::NonNull;

use thiserror::Error;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Memory::{
    CreateFileMappingA, FILE_MAP_READ, FILE_MAP_WRITE, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile,
    OpenFileMappingA, PAGE_READWRITE, UnmapViewOfFile,
};
use windows::Win32::System::Threading::CreateEventA;
use windows::core::PCSTR;

#[allow(clippy::enum_variant_names)]
#[derive(Error, Debug)]
pub enum SharedMemoryError {
    #[error("Failed to open shared memory '{name}': not found or inaccessible")]
    OpenFailed { name: String },

    #[error("Failed to create shared memory '{name}'")]
    CreateFailed { name: String },

    #[error("Failed to map view of shared memory '{name}'")]
    MapFailed { name: String },

    #[error("Failed to create event '{name}'")]
    EventCreateFailed { name: String },
}

/// A read-only view into shared memory created by another process.
pub struct SharedMemoryReader {
    handle: HANDLE,
    view: NonNull<u8>,
    size: usize,
}

impl SharedMemoryReader {
    pub fn open(name: &str, size: usize) -> Result<Self, SharedMemoryError> {
        let name_cstr = CString::new(name).map_err(|_| SharedMemoryError::OpenFailed {
            name: name.to_string(),
        })?;

        // Open existing file mapping
        let handle = unsafe {
            OpenFileMappingA(
                FILE_MAP_READ.0,
                false,
                PCSTR::from_raw(name_cstr.as_ptr() as *const u8),
            )
        }
        .map_err(|_| SharedMemoryError::OpenFailed {
            name: name.to_string(),
        })?;

        let view = unsafe { MapViewOfFile(handle, FILE_MAP_READ, 0, 0, 0) };

        if view.Value.is_null() {
            unsafe { CloseHandle(handle).ok() };
            return Err(SharedMemoryError::MapFailed {
                name: name.to_string(),
            });
        }

        Ok(Self {
            handle,
            view: NonNull::new(view.Value as *mut u8).unwrap(),
            size,
        })
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.view.as_ptr()
    }

    #[allow(dead_code)]
    pub fn size(&self) -> usize {
        self.size
    }
}

impl Drop for SharedMemoryReader {
    fn drop(&mut self) {
        unsafe {
            UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                Value: self.view.as_ptr() as *mut _,
            })
            .ok();
            CloseHandle(self.handle).ok();
        }
    }
}

pub struct SharedMemoryWriter {
    handle: HANDLE,
    view: NonNull<u8>,
    size: usize,
}

impl SharedMemoryWriter {
    pub fn create(name: &str, size: usize) -> Result<Self, SharedMemoryError> {
        let name_cstr = CString::new(name).map_err(|_| SharedMemoryError::CreateFailed {
            name: name.to_string(),
        })?;

        let handle = unsafe {
            CreateFileMappingA(
                HANDLE::default(),
                None,
                PAGE_READWRITE,
                0,
                size as u32,
                PCSTR::from_raw(name_cstr.as_ptr() as *const u8),
            )
        }
        .map_err(|_| SharedMemoryError::CreateFailed {
            name: name.to_string(),
        })?;

        let view = unsafe { MapViewOfFile(handle, FILE_MAP_WRITE, 0, 0, size) };

        if view.Value.is_null() {
            unsafe { CloseHandle(handle).ok() };
            return Err(SharedMemoryError::MapFailed {
                name: name.to_string(),
            });
        }

        unsafe {
            std::ptr::write_bytes(view.Value as *mut u8, 0, size);
        }

        Ok(Self {
            handle,
            view: NonNull::new(view.Value as *mut u8).unwrap(),
            size,
        })
    }

    pub unsafe fn write(&mut self, offset: usize, data: &[u8]) {
        debug_assert!(offset + data.len() <= self.size);
        unsafe {
            std::ptr::copy_nonoverlapping(
                data.as_ptr(),
                self.view.as_ptr().add(offset),
                data.len(),
            );
        }
    }

    #[allow(dead_code)]
    pub fn size(&self) -> usize {
        self.size
    }
}

impl Drop for SharedMemoryWriter {
    fn drop(&mut self) {
        unsafe {
            UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                Value: self.view.as_ptr() as *mut _,
            })
            .ok();
            CloseHandle(self.handle).ok();
        }
    }
}

pub struct EventHandle {
    handle: HANDLE,
}

impl EventHandle {
    pub fn create(name: &str) -> Result<Self, SharedMemoryError> {
        let name_cstr = CString::new(name).map_err(|_| SharedMemoryError::EventCreateFailed {
            name: name.to_string(),
        })?;

        let handle = unsafe {
            CreateEventA(
                None,
                false, // manual reset
                false, // initial state
                PCSTR::from_raw(name_cstr.as_ptr() as *const u8),
            )
        }
        .map_err(|_| SharedMemoryError::EventCreateFailed {
            name: name.to_string(),
        })?;

        Ok(Self { handle })
    }
}

impl Drop for EventHandle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle).ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_read_shared_memory() {
        let name = "Local\\KsanaTestShm";
        let size = 1024;
        let test_data = b"Hello, shared memory!";

        {
            // Create writer
            let mut writer = SharedMemoryWriter::create(name, size).unwrap();
            assert_eq!(writer.size(), size);

            // Write some data
            unsafe {
                writer.write(0, test_data);
            }

            // Open reader to the same region
            let reader = SharedMemoryReader::open(name, size).unwrap();
            assert_eq!(reader.size(), size);

            // Verify data
            unsafe {
                let slice = std::slice::from_raw_parts(reader.as_ptr(), reader.size());
                assert_eq!(&slice[..test_data.len()], test_data);
            }
        }

        // When writer goes out of scope, shared memory is cleaned up
        // So opening it with a reader should fail
        let reader = SharedMemoryReader::open(name, size);
        assert!(reader.is_err());
    }

    #[test]
    fn test_write_at_offset() {
        let name = "Local\\KsanaTestShmOffset";
        let size = 1024;

        let mut writer = SharedMemoryWriter::create(name, size).unwrap();

        unsafe {
            writer.write(100, b"data at offset");
        }

        let reader = SharedMemoryReader::open(name, size).unwrap();

        unsafe {
            let slice = std::slice::from_raw_parts(reader.as_ptr(), reader.size());
            assert_eq!(&slice[100..114], b"data at offset");
        }
    }

    #[test]
    fn test_open_nonexistent_fails() {
        let result = SharedMemoryReader::open("Local\\NonexistentShm12345", 1024);
        assert!(matches!(result, Err(SharedMemoryError::OpenFailed { .. })));
    }
}
