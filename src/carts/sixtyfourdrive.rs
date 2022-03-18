
use std::cmp::min;
use bytes::{BufMut, BytesMut};
use libftd2xx::{Ftdi, FtdiCommon};
use crate::carts::{Cartridge, Model, Segment, Error::*, Result};
use crate::detect_model;

pub const CMD_LOAD_FROM_PC: u8 = 0x20;
pub const CMD_DUMP_TO_PC: u8 = 0x30;
pub const CMD_TARGET_SIDE_FIFO: u8 = 0x40;
pub const CMD_SET_SAVE_TYPE: u8 = 0x70;
pub const CMD_SET_CIC_TYPE: u8 = 0x72;
pub const CMD_SET_CI_EXTENDED: u8 = 0x74;
pub const CMD_VERSION_REQUEST: u8 = 0x80;
pub const CMD_UPGRADE_START: u8 = 0x84;
pub const CMD_UPGRADE_REPORT: u8 = 0x85;



pub struct SixtyFourDrive {
    device: Ftdi,
    model: Model,
}
impl Cartridge for SixtyFourDrive {
    fn new(mut device: Ftdi) -> Result<Box<dyn Cartridge>> where Self: Sized {
        let info = match device.device_info() {
            Ok(info) => info,
            Err(err) => return Err(FtdiStatusError(err))
        };
        let model = match detect_model(&info) {
            Ok(model) => model,
            Err(err) => return Err(err)
        };
        
        if model != Model::SixtyFourDriveHW1 && model != Model::SixtyFourDriveHW2 {
            return Err(ModelDetectFailed)
        }
        
        Ok(Box::new(Self { device, model }))
    }
    
    fn upload(&mut self, segment: Segment, offset: u32, data: Vec<u8>) -> Result<()> {
        const SIZE: u32 = 0x800000;
        
        let chunks = data.len() / SIZE as usize;
        let bank = bank_index(&segment, &self.model, false); //TODO detect stadium 2
        
        let mut data_index = 0;
        for i in 0..chunks {
            let mut packet = BytesMut::new();
            packet.put_slice(&command_packet(CMD_LOAD_FROM_PC));
            packet.put_u32(offset + (i as u32 * SIZE));
            
            let length = min(data.len() - data_index, SIZE as usize);
            packet.put_u32(bank | (length as u32 & 0x00FFFFFF));
            
            packet.put_slice(&data[data_index..(data_index + length)]);
            data_index += length;
            
            println!("Uploading data. offset: {:#010X}, banklen: {:#010X}", offset + (i as u32 * SIZE), bank | (length as u32 & 0x00FFFFFF));
            match self.device.write_all(&packet) {
                Ok(_) => (),
                Err(err) => return Err(FtdiTimeoutError(err))
            }
            
            println!("Write complete.");
            match check_error(&mut self.device, CMD_LOAD_FROM_PC) {
                Ok(_) => (),
                Err(err) => return Err(err)
            }
        }
        
        println!("Upload complete!");
        Ok(())
    }
    
    fn download(&mut self, segment: Segment, offset: u32) -> Result<Vec<u8>> {
        todo!()
    }
    
    fn model(&mut self) -> Model {
        self.model.clone()
    }
    
    fn device(&mut self) -> &mut Ftdi {
        &mut self.device
    }
}


#[inline]
pub fn command_packet(id: u8) -> [u8; 4] {
    [id, 0x43, 0x4D, 0x44]
}

#[inline]
pub fn complete_packet(id: u8) -> [u8; 4] {
    [0x43, 0x4D, 0x50, id]
}

pub fn check_error(device: &mut Ftdi, id: u8) -> Result<()> {
    let mut buf = [0u8; 4];
    match device.read_all(&mut buf) {
        Ok(_) => (),
        Err(err) => return Err(FtdiTimeoutError(err))
    }
    
    if buf == complete_packet(id) {
        Ok(())
    } else {
        Err(CommandFailed(id))
    }
}

pub fn bank_index(segment: &Segment, model: &Model, is_stadium: bool) -> u32 {
    use Segment::*;
    
    (match segment {
        Rom => 1,
        Sram256 => 2,
        Sram768 => 3,
        FlashRam if model == &Model::SixtyFourDriveHW1 && is_stadium => 5,
        FlashRam => 4,
        Eeprom4 => 6,
        Eeprom16 => 6,
    } << 24)
}