use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use crc::{Crc, CRC_32_ISO_HDLC};
use log::debug;

pub const CRC: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

pub mod sixtyfourdrive;

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
impl FromStr for Cic {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use Cic::*;
        
        Ok(match s.to_lowercase().as_str() {
            "auto" => Auto,
            "6101" => Var6101,
            "6102" => Var6102,
            "7101" => Var7101,
            "7102" => Var7102,
            "x103" => VarX103,
            "x105" => VarX105,
            "x106" => VarX106,
            "5101" => Var5101,
            
            _ => return Err("Accepted values: auto, 6101, 6102, 7101, 7102, x103, x105, x106, or 5101".into())
        })
    }
}

impl Cic {
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
impl FromStr for SaveType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use SaveType::*;
        
        Ok(match s.to_lowercase().as_str() {
            "auto" => Auto,
            "eeprom4kbit" => Eeprom4Kbit,
            "eeprom16kbit" => Eeprom16Kbit,
            "sram256kbit" => Sram256Kbit,
            "flashram1mbit" => FlashRam1Mbit,
            "sram768kbit" => Sram768Kbit,
            "pokestadium2" => FlashRam1MbitStadium,
            "none" | "nothing" => Nothing,
            
            _ => return Err("Accepted values: auto, eeprom4kbit, eeprom16kbit, sram256kbit, flashram1mbit, sram768kbit, pokestadium2, or none".into())
        })
    }
}
impl SaveType {
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
}
