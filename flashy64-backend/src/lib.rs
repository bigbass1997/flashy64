extern crate core;
#[macro_use]
extern crate lazy_static;

use libftd2xx::{DeviceInfo, Ftdi, FtdiCommon, FtStatus, TimeoutError};
use log::debug;
use crate::carts::{Cic, SaveType};
use crate::carts::sixtyfourdrive::SixtyFourDrive;
use crate::unfloader::DebugResponse;

pub mod carts;
pub mod unfloader;

#[derive(Debug, PartialEq)]
pub enum Error {
    FtdiStatus(FtStatus),
    FtdiTimeout(TimeoutError),
    
    CommunicationFailed(String),
    
    Unsupported,
}
impl From<FtStatus> for Error {
    fn from(value: FtStatus) -> Self {
        Self::FtdiStatus(value)
    }
}
impl From<TimeoutError> for Error {
    fn from(value: TimeoutError) -> Self {
        Self::FtdiTimeout(value)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub trait Flashcart {
    fn upload_rom(&mut self, data: &[u8]) -> Result<()>;
    fn download_rom(&mut self, length: u32) -> Result<Vec<u8>>;
    
    fn set_cic(&mut self, cic: Cic) -> Result<()>;
    fn set_savetype(&mut self, savetype: SaveType) -> Result<()>;
    
    fn recv_debug(&mut self) -> Result<DebugResponse>;
    fn send_debug(&mut self) -> Result<()>;
    fn info(&mut self) -> Result<DeviceInfo>;
}


pub fn carts() -> Result<Vec<Box<dyn Flashcart>>> {
    let mut carts = vec![];
    
    for info in libftd2xx::list_devices()? {
        debug!("Device detected: {info:?}");
        match from_info(&info) {
            Ok(cart) => carts.push(cart),
            Err(_) => (),
        }
    }
    
    Ok(carts)
}

pub fn from_serial<S: AsRef<str>>(serial: S) -> Result<Box<dyn Flashcart>> {
    let mut device = Ftdi::with_serial_number(serial.as_ref())?;
    let info = device.device_info()?;
    device.close()?;
    
    from_info(&info)
}

pub fn from_info(info: &DeviceInfo) -> Result<Box<dyn Flashcart>> {
    match (info.vendor_id, info.product_id, info.description.as_str()) {
        (0x0403, 0x6010, "64drive USB device A") => Ok(Box::new(SixtyFourDrive::new(Ftdi::with_serial_number(&info.serial_number)?)?)),
        (0x0403, 0x6014, "64drive USB device") => Ok(Box::new(SixtyFourDrive::new(Ftdi::with_serial_number(&info.serial_number)?)?)),
        (0x0403, 0x6001, "FT245R USB FIFO") => todo!("everdrive"),
        (0x0403, 0x6014, "SC64") => todo!("summercart64"),
        
        _ => Err(Error::Unsupported)
    }
}