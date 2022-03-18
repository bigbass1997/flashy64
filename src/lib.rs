extern crate core;

use libftd2xx::{DeviceInfo, Ftdi, FtdiCommon};
use crate::carts::{Cartridge, Error, Model, Model::*};
use crate::carts::sixtyfourdrive::SixtyFourDrive;

pub mod carts;

pub fn list_carts() -> Result<Vec<Box<dyn Cartridge>>, Error> {
    let devices = match libftd2xx::list_devices() {
        Ok(devices) => devices,
        Err(err) => return Err(Error::FtdiStatusError(err))
    };
    
    let mut carts = vec![];
    for info in devices {
        println!("info: {:?}", info);
        if info.port_open {
            continue
        }
        
        if let Ok(cart) = cart(Ftdi::with_serial_number(&info.serial_number).unwrap()) {
            carts.push(cart);
        }
    }
    
    Ok(carts)
}

pub fn cart(mut device: Ftdi) -> Result<Box<dyn Cartridge>, Error> {
    let info = device.device_info().unwrap();
    
    let model = match detect_model(&info) {
        Ok(model) => model,
        Err(err) => return Err(err)
    };
    
    match model {
        SixtyFourDriveHW1 | SixtyFourDriveHW2 => SixtyFourDrive::new(device),
        Everdrive3 => unimplemented!(),
        EverdriveX7 => unimplemented!(),
        SummerCart64 => unimplemented!(),
        Other(_) => unimplemented!(),
    }
}

pub fn detect_model(info: &DeviceInfo) -> Result<Model, Error> {
    match (info.vendor_id, info.product_id, info.description.as_str()) {
        (0x0403, 0x6010, "64drive USB device A") => Ok(SixtyFourDriveHW1), // 64drive HW1
        (0x0403, 0x6014, "64drive USB device") => Ok(SixtyFourDriveHW2), // 64drive HW2
        (0x0403, 0x6001, "FT245R USB FIFO") => Ok(EverdriveX7), // Everdrive xx
        (0x0403, 0x6014, "SummerCart64") => Ok(SummerCart64), // SummerCart64
        _ => Err(Error::ModelDetectFailed)
    }
}