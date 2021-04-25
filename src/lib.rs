use nix::sys::mman;
use nix::errno::Errno;

use std::ffi::c_void;
use std::fmt::Display;
use std::fs::OpenOptions;
use std::os::unix::io::{AsRawFd, RawFd};
use std::path::PathBuf;


fn detect_peripheral_base() -> Result<i64, Error> {
    // Stub that works for the Pi4
    let start: i64 = 0xfe200000;
    return Ok(start);
}


fn open_file(path: impl Into<PathBuf>) -> Result<std::fs::File, Error> {
    let path = path.into();
    let file = OpenOptions::new()
                .create(false)
                .read(true)
                .write(true)
                .open(&path)
                .map_err(|e| Error::from_io(format!("failed to open {}", path.display()), e))?;
    Ok(file)
}



pub struct Error {
    pub message: String,
    pub errno: Option<Errno>,
}

impl Error {
    pub fn new (message: impl std::string::ToString, errno: Option<Errno>) -> Self {
        Self {
            message: message.to_string(),
            errno,
        }
    }

    pub fn from_nix(message: impl std::string::ToString, error: nix::Error) -> Self {
        Self::new(message, error.as_errno())
    }

    pub fn from_io(message: impl std::string::ToString, error: std::io::Error) -> Self {
        Self::new(message, error.raw_os_error().map(Errno::from_i32))
    }
}


impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self.errno {
            None => write!(f, "{}", self.message),
            Some(errno) => write!(f, "{}: {}", self.message, errno.desc())
        }
    }    
}

const GPIO_BLOCK_SIZE: usize = 0x100;

pub struct GPIO {
    buffer: *mut c_void,
}

impl GPIO {

 	pub fn new() -> Result<Self, Error> {
        let fp: std::fs::File  = open_file("/dev/mem")?;
        let fd: RawFd = fp.as_raw_fd();
        let gpio_offset:i64 = detect_peripheral_base()?;
        let ptr = unsafe {
            mman::mmap(std::ptr::null_mut(), GPIO_BLOCK_SIZE, 
                mman::ProtFlags::PROT_READ | mman::ProtFlags::PROT_WRITE,
                mman::MapFlags::MAP_SHARED, fd, gpio_offset)
                .map_err(
                    |e| Error::from_nix(format!(
                    "failed to map the GPIO ({:#X}) from /dev/mem ", gpio_offset), e))?
        };
        Ok(Self{buffer: ptr})
    }

}


impl Drop for GPIO {
    fn drop(&mut self) {
        unsafe {
            drop(mman::munmap(self.buffer, GPIO_BLOCK_SIZE))
        }
    }
}


#[cfg(test)]
mod tests {
    //use super::*;
}