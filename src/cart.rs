use std::cmp::min;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::time::Duration;
use bytes::{BufMut, BytesMut};
use crc::{Crc, CRC_32_ISO_HDLC};
use libftd2xx::{BitMode, Ftdi, FtdiCommon, FtStatus, TimeoutError};
use log::debug;


pub const CMD_LOAD_FROM_PC: u8 = 0x20;
pub const CMD_DUMP_TO_PC: u8 = 0x30;
pub const CMD_TARGET_SIDE_FIFO: u8 = 0x40;
pub const CMD_SET_SAVE_TYPE: u8 = 0x70;
pub const CMD_SET_CIC_TYPE: u8 = 0x72;
pub const CMD_SET_CI_EXTENDED: u8 = 0x74;
pub const CMD_VERSION_REQUEST: u8 = 0x80;
pub const CMD_UPGRADE_START: u8 = 0x84;
pub const CMD_UPGRADE_REPORT: u8 = 0x85;

pub const CRC: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

lazy_static! {
    pub static ref ROMDB: HashMap<String, SaveType> = {
        #[derive(Clone, Debug, PartialEq, Default)]
        pub struct DbEntry {
            pub md5: String,
            pub savetype: SaveType
        }
        
        let mut entries = HashMap::new();
        
        let mut lines = include_str!("romdb.ini").lines();
        let mut entry = None;
        while let Some(line) = lines.next() {
            if line.starts_with("[") {
                entry = Some(DbEntry::default());
                
                let mut tmp = entry.unwrap();
                tmp.md5 = line.trim_matches('[').trim_matches(']').to_string();
                
                entry = Some(tmp);
            } else if line.starts_with("SaveType=") {
                let mut tmp = entry.unwrap();
                tmp.savetype = match line.split_once('=').unwrap().1 {
                    "None" => SaveType::Nothing,
                    "SRAM" => SaveType::Sram256Kbit,
                    "Eeprom 4KB" => SaveType::Eeprom4Kbit,
                    "Eeprom 16KB" => SaveType::Eeprom16Kbit,
                    "Flash RAM" => SaveType::FlashRam1Mbit,
                    _ => SaveType::Unknown
                };
                
                entry = Some(tmp);
            } else if line.starts_with("GoodName=") {
                let mut tmp = entry.unwrap();
                let goodname = line.split_once('=').unwrap().1;
                if goodname.contains("Dezaemon 3D") {
                    tmp.savetype = SaveType::Sram768Kbit;
                } else if goodname.contains("Pokemon Stadium 2") {
                    tmp.savetype = SaveType::FlashRam1MbitStadium;
                }
                
                entry = Some(tmp);
            } else if line.is_empty() && entry.is_some() {
                let tmp = entry.unwrap();
                entries.insert(tmp.md5, tmp.savetype);
                
                entry = None;
            }
        }
        
        entries
    };
}


#[derive(Debug, PartialEq)]
pub enum Error {
    FtdiStatus(FtStatus),
    FtdiTimeout(TimeoutError),
    InvalidEndpoints,
    ModelDetectFailed,
    UnsupportedOperation(String),
    CommandFailed(u8),
}
use Error::*;

pub type Result<T> = std::result::Result<T, Error>;


#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Segment {
    Rom,
    Sram256,
    Sram768,
    FlashRam,
    Eeprom4,
    Eeprom16,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Model {
    HW1, HW2,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Cic {
    Auto,
    Var6101,
    Var6102,
    Var7101,
    Var7102,
    VarX103,
    VarX105,
    VarX106,
    Var5101,
    Unknown,
}
impl Display for Cic {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use Cic::*;
        
        write!(f, "{}", match self {
            Auto => "auto",
            Var6101 => "6101",
            Var6102 => "6102",
            Var7101 => "7101",
            Var7102 => "7102",
            VarX103 => "x103",
            VarX105 => "x105",
            VarX106 => "x106",
            Var5101 => "5101",
            Unknown => "unknown",
        })
    }
}
impl Cic {
    pub fn from_str<'a>(s: impl Into<&'a str>) -> Cic {
        use Cic::*;
        
        match s.into().to_lowercase().as_str() {
            "auto" => Auto,
            "6101" => Var6101,
            "6102" => Var6102,
            "7101" => Var7101,
            "7102" => Var7102,
            "x103" => VarX103,
            "x105" => VarX105,
            "x106" => VarX106,
            "5101" => Var5101,
            _ => Unknown
        }
    }
    
    /// Attempts to detect which CIC variant matches the provided ROM.
    /// 
    /// If ROM does not include standard 0x40 byte header, or is smaller than 0x1000 bytes, this method
    /// will fail.
    pub fn from_rom(data: &[u8]) -> Cic {
        if data.len() < 0x1000 { return Cic::Unknown }
        
        Self::from_ipl3(&data[0x40..0x1000])
    }
    
    /// Attempts to detect which CIC variant matches the provided IPL3.
    /// 
    /// Data slice should NOT include the ROM header. Only data from rom offset 0x40 to 0x1000 (exclusive).
    pub fn from_ipl3(data: &[u8]) -> Cic {
        use Cic::*;
        
        let sum = CRC.checksum(data);
        debug!("Calculated IPL3 CRC: {:#010X}", sum);
        match sum {
            0x6170A4A1 => Var6101,
            0x90BB6CB5 => Var6102,
            0x009E9EA3 => Var7102,
            0x0B050EE0 => VarX103,
            0x98BC2C86 => VarX105,
            0xACC8580A => VarX106,
            _ => Unknown
        }
    }
    
    /// Gets the 64drive index value associated with each CIC variant.
    /// 
    /// `Cic::Auto` and `Cic::Unknown` will return `None`.
    pub fn index(&self) -> Option<u8> {
        use Cic::*;
        
        match self {
            Var6101 => Some(0),
            Var6102 => Some(1),
            Var7101 => Some(2),
            Var7102 => Some(3),
            VarX103 => Some(4),
            VarX105 => Some(5),
            VarX106 => Some(6),
            Var5101 => Some(7),
            Auto | Unknown => None
        }
    }
}


#[derive(Copy, Clone, Debug, PartialEq)]
pub enum SaveType {
    Auto,
    Nothing,
    Eeprom4Kbit,
    Eeprom16Kbit,
    Sram256Kbit,
    FlashRam1Mbit,
    Sram768Kbit,
    FlashRam1MbitStadium,
    Unknown,
}
impl Default for SaveType {
    fn default() -> Self {
        SaveType::Nothing
    }
}
impl SaveType {
    pub fn from_str<'a>(s: impl Into<&'a str>) -> SaveType {
        use SaveType::*;
        
        match s.into().to_lowercase().as_str() {
            "auto" => Auto,
            "eeprom4kbit" => Eeprom4Kbit,
            "eeprom16kbit" => Eeprom16Kbit,
            "sram256kbit" => Sram256Kbit,
            "flashram1mbit" => FlashRam1Mbit,
            "sram768kbit" => Sram768Kbit,
            "pokestadium2" => FlashRam1MbitStadium,
            _ => Nothing
        }
    }
    
    pub fn from_rom(data: &[u8]) -> SaveType {
        let hash = md5::compute(data).0;
        let mut hash_str = String::new();
        for byte in hash {
            hash_str.push_str(&format!("{:02X}", byte));
        }
        debug!("Calculated ROM Hash: {}", hash_str);
        
        match ROMDB.get(&hash_str) {
            Some(savetype) => *savetype,
            None => SaveType::Unknown,
        }
    }
    
    pub fn index(&self) -> Option<u8> {
        use SaveType::*;
        
        match self {
            Nothing => Some(0),
            Eeprom4Kbit => Some(1),
            Eeprom16Kbit => Some(2),
            Sram256Kbit => Some(3),
            FlashRam1Mbit => Some(4),
            Sram768Kbit => Some(5),
            FlashRam1MbitStadium => Some(6),
            Auto | Unknown => None,
        }
    }
}


#[derive(Debug)]
pub struct SixtyFourDrive {
    device: Ftdi,
    model: Model,
}
impl SixtyFourDrive {
    pub fn new(mut device: Ftdi) -> Result<Self> {
        let model = match model(&mut device) {
            Ok(model) => model,
            Err(err) => return Err(err)
        };
        
        device.set_bit_mode(0xFF, BitMode::Reset).unwrap();
        device.set_bit_mode(0xFF, BitMode::SyncFifo).unwrap();
        device.set_timeouts(Duration::from_secs(10), Duration::from_secs(10)).unwrap();
        
        Ok(Self {
            device,
            model,
        })
    }
    
    pub fn upload(&mut self, segment: Segment, offset: u32, data: Vec<u8>) -> Result<()> {
        const SIZE: u32 = 0x800000;
        
        let chunks = (data.len() as f32 / SIZE as f32).ceil() as u32;
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
            
            debug!("Uploading data. offset: {:#010X}, banklen: {:#010X}", offset + (i as u32 * SIZE), bank | (length as u32 & 0x00FFFFFF));
            match self.device.write_all(&packet) {
                Ok(_) => (),
                Err(err) => return Err(FtdiTimeout(err))
            }
            
            debug!("Write complete.");
            match self.check_error(CMD_LOAD_FROM_PC) {
                Ok(_) => (),
                Err(err) => return Err(err)
            }
        }
        
        debug!("Upload complete!");
        Ok(())
    }
    
    pub fn download(&mut self, segment: Segment, offset: u32, length: u32) -> Result<Vec<u8>> {
        const SIZE: u32 = 0x20000;
        
        let chunks = (length as f32 / SIZE as f32).ceil() as u32;
        let bank = bank_index(&segment, &self.model, false); //TODO detect stadium 2
        
        let mut data = vec![];
        let mut data_index = 0;
        for i in 0..chunks {
            let mut packet = BytesMut::new();
            packet.put_slice(&command_packet(CMD_DUMP_TO_PC));
            packet.put_u32(offset + (i * SIZE));
            
            let length = min(length - data_index, SIZE);
            if (length & 0x00FFFFFF) < 4 { break }
            packet.put_u32(bank | (length & 0x00FFFFFF));
            
            data_index += length;
            
            debug!("Downloading data. offset: {:#010X}, banklen: {:#010X}", offset + (i as u32 * SIZE), bank | (length as u32 & 0x00FFFFFF));
            match self.device.write_all(&packet) {
                Ok(_) => (),
                Err(err) => return Err(FtdiTimeout(err))
            }
            
            let mut buf = vec![0u8; length as usize];
            match self.device.read_all(buf.as_mut_slice()) {
                Ok(_) => (),
                Err(err) => return Err(FtdiTimeout(err))
            }
            
            debug!("Read complete.");
            match self.check_error(CMD_DUMP_TO_PC) {
                Ok(_) => (),
                Err(err) => return Err(err)
            }
            
            data.append(&mut buf);
        }
        
        debug!("Download complete! {:.4} MiB", data.len() as f32 / (1024.0 * 1024.0));
        Ok(data)
    }
    
    pub fn cic(&mut self, cic_index: u8) -> Result<()> {
        let mut packet = BytesMut::new();
        packet.put_slice(&command_packet(CMD_SET_CIC_TYPE));
        packet.put_u32(0x80000000 | (cic_index & 0x7) as u32);
        
        match self.device.write_all(&packet) {
            Ok(_) => (),
            Err(err) => return Err(FtdiTimeout(err))
        }
        
        match self.check_error(CMD_SET_CIC_TYPE) {
            Ok(_) => (),
            Err(err) => return Err(err)
        }
        
        debug!("CIC is set {:#010X}", 0x80000000 | (cic_index & 0x7) as u32);
        
        Ok(())
    }
    
    pub fn savetype(&mut self, savetype_index: u8) -> Result<()> {
        let mut packet = BytesMut::new();
        packet.put_slice(&command_packet(CMD_SET_SAVE_TYPE));
        packet.put_u32((savetype_index as u32) & 0x0000000F);
        
        match self.device.write_all(&packet) {
            Ok(_) => (),
            Err(err) => return Err(FtdiTimeout(err))
        }
        
        match self.check_error(CMD_SET_SAVE_TYPE) {
            Ok(_) => (),
            Err(err) => return Err(err)
        }
        
        debug!("SaveType is set {:#010X}", (savetype_index as u32) & 0x0000000F);
        Ok(())
    }
    
    pub fn device(&mut self) -> &mut Ftdi {
        &mut self.device
    }
    
    pub fn check_error(&mut self, id: u8) -> Result<()> {
        let mut buf = [255u8; 4];
        
        match self.device.read_all(&mut buf) {
            Ok(_) => (),
            Err(err) => return Err(FtdiTimeout(err))
        }
        
        if buf == complete_packet(id) {
            Ok(())
        } else {
            Err(CommandFailed(id))
        }
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


pub fn bank_index(segment: &Segment, model: &Model, is_stadium: bool) -> u32 {
    use Segment::*;
    
    (match segment {
        Rom => 1,
        Sram256 => 2,
        Sram768 => 3,
        FlashRam if model == &Model::HW1 && is_stadium => 5,
        FlashRam => 4,
        Eeprom4 => 6,
        Eeprom16 => 6,
    } << 24)
}

pub fn model(device: &mut Ftdi) -> Result<Model> {
    let info = match device.device_info() {
        Ok(info) => info,
        Err(err) => return Err(FtdiStatus(err))
    };
    
    match (info.vendor_id, info.product_id, info.description.as_str()) {
        (0x0403, 0x6010, "64drive USB device A") => Ok(Model::HW1),
        (0x0403, 0x6014, "64drive USB device") => Ok(Model::HW2),
        _ => Err(Error::ModelDetectFailed)
    }
}