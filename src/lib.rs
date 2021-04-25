use nix::sys::mman;
use nix::errno::Errno;

use std::ffi::c_void;
use std::fmt::Display;
use std::fs::OpenOptions;
use std::os::unix::io::{AsRawFd, RawFd};
use std::path::PathBuf;


pub enum Register {
    GPFSEL = 0x00,
    
    GPSET =  0x1c,
    GPCLR =  0x28,
    GPLEV =  0x34,
    GPEDS =  0x40,
    GPREN =  0x4c,
    GPFEN =  0x58,
    GPHEN =  0x64,
    GPLEN =  0x70,
    GPAREN = 0x7c,
    GPAFEN = 0x88,

    GPPUPPDNCNTRL = 0xe4,
}


const REGISTER_SIZE: u32 = 4;
const GPIO_PIN_COUNT: u32 = 58;
const GPIO_FUNCS_PER_REGISTER: u32 = 10;
const GPIO_PUPPUD_PER_REGISTER: u32 = 16;

fn assert_pin_index(pin: u32) {
    assert!(
        pin < GPIO_PIN_COUNT,
        "Illegal pin value. Pin must be in [0,{pin_count}) - Paniced on pin = {pin}",
        pin_count = GPIO_PIN_COUNT, pin = pin
    );
}

macro_rules! register_offset {
    ($pin:expr) => {
        if $pin > 31 { 4 } else { 0 }
    };
}

impl Register {

    pub fn to_offset(self, pin: u32) -> usize {
        assert_pin_index(pin);

        match self {
            Register::GPFSEL => Register::gpfsel_offset_for(pin),
            Register::GPPUPPDNCNTRL => Register::gp_pullup_pulldown(pin),
            _ => Register::gp2reg_offset_for(self as u32, pin),
        }
    }

    fn gpfsel_offset_for(pin: u32) -> usize {
        ((pin / GPIO_FUNCS_PER_REGISTER) * REGISTER_SIZE) as usize
    }

    fn gp_pullup_pulldown(pin: u32) -> usize {
        (Register::GPPUPPDNCNTRL as usize) + 
            ((pin / GPIO_PUPPUD_PER_REGISTER) * REGISTER_SIZE) as usize
    }

    fn gp2reg_offset_for(offset: u32, pin: u32) -> usize {
        (offset + register_offset!(pin)) as usize
    }

}


#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum PinFunction {
	Input = 0b000,
	Output = 0b001,
	Alt0 = 0b100,
	Alt1 = 0b101,
	Alt2 = 0b110,
	Alt3 = 0b111,
	Alt4 = 0b011,
	Alt5 = 0b010,
}

impl PinFunction {

	pub fn to_bits(&self, pin: u32) -> u32 {
		let fval = *self as u32;
		fval <<	((pin % 10) * 3)
	}

	pub fn clear_mask(pin: u32) -> u32 {
		!(0b111 << ((pin % 10) * 3))
	}
}


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
    use super::*;

    #[test]
    fn test_register_gpfsel_to_offset() {
        let gpfsel0 = Register::GPFSEL;
        let gpfsel1 = Register::GPFSEL;
        let gpfsel2 = Register::GPFSEL;
        let gpfsel3 = Register::GPFSEL;
        let gpfsel4 = Register::GPFSEL;
        let gpfsel5 = Register::GPFSEL;
        
        assert_eq!(gpfsel0.to_offset(5),  0x00);
        assert_eq!(gpfsel1.to_offset(15), 0x04);
        assert_eq!(gpfsel2.to_offset(25), 0x08);
        assert_eq!(gpfsel3.to_offset(35), 0x0c);
        assert_eq!(gpfsel4.to_offset(45), 0x10);
        assert_eq!(gpfsel5.to_offset(55), 0x14);
    }


    #[test]
    fn test_register_2regcntrl_to_offset() {
        let gpset0 = Register::GPSET;
        let gpset1 = Register::GPSET;

        assert_eq!(gpset0.to_offset(5),  0x1c + 0x00);
        assert_eq!(gpset1.to_offset(45), 0x1c + 0x04);
    }


    #[test]
    fn test_pullup_pulldown_control_to_offset() {
        let gp_pup_pud0 = Register::GPPUPPDNCNTRL;
        let gp_pup_pud1 = Register::GPPUPPDNCNTRL;
        let gp_pup_pud2 = Register::GPPUPPDNCNTRL;
        let gp_pup_pud3 = Register::GPPUPPDNCNTRL;
    
        assert_eq!(gp_pup_pud0.to_offset(8),    0xe4 + 0x00);
        assert_eq!(gp_pup_pud1.to_offset(8+16), 0xe4 + 0x04);
        assert_eq!(gp_pup_pud2.to_offset(8+32), 0xe4 + 0x08);
        assert_eq!(gp_pup_pud3.to_offset(8+48), 0xe4 + 0x0c);

    }


    #[test]
    fn test_pinfunction_to_bits() {
        let pin32: u32 = 32;
        let pin5: u32 = 5;
        let output = PinFunction::Output;

        assert_eq!(output.to_bits(pin32),         0b001000000);
        assert_eq!(output.to_bits(pin5), 0b001000000000000000);
    }

    #[test]
    fn test_pinfunction_clear_mask() {
        let pin32: u32 = 32;
        let pin5: u32 = 5;

        let mask32: u32 = !(0b111000000);
        let mask5: u32 = !(0b111000000000000000); 
        assert_eq!(PinFunction::clear_mask(pin32), mask32);
        assert_eq!(PinFunction::clear_mask(pin5), mask5);

    }
}