extern crate core;
#[macro_use]
extern crate lazy_static;

use libftd2xx::Ftdi;
use log::debug;
use crate::cart::{Error, SixtyFourDrive};

pub mod cart;

pub fn list_carts() -> Result<Vec<SixtyFourDrive>, Error> {
    let devices = match libftd2xx::list_devices() {
        Ok(devices) => devices,
        Err(err) => return Err(Error::FtdiStatus(err))
    };
    
    let mut carts = vec![];
    for info in devices {
        debug!("Device Found: {:?}", info);
        if info.port_open {
            continue
        }
        
        if let Ok(cart) = SixtyFourDrive::new(Ftdi::with_serial_number(&info.serial_number).unwrap()) {
            carts.push(cart);
        }
    }
    
    Ok(carts)
}