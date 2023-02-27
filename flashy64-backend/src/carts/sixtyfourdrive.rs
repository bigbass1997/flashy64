use std::cmp::min;
use std::time::Duration;
use bytes::{BufMut, BytesMut};
use libftd2xx::{BitMode, DeviceInfo, Ftdi, FtdiCommon};
use log::debug;
use crate::{Error, Flashcart, Result};
use crate::carts::{Cic, SaveType};
use crate::Error::CommunicationFailed;
use crate::unfloader::{DataType, DebugResponse};

#[derive(Clone, PartialEq, Debug)]
pub enum Command {
    LoadFromPc {
        addr: u32,
        bank_id_len: u32,
        data: Vec<u8>,
    },
    DumpToPc {
        addr: u32,
        bank_id_len: u32,
    },
    TargetSideFifo(Vec<u8>),
    SetSaveType(SaveType),
    SetCicType(Cic),
    SetCiExtended(u32),
    VersionRequest,
}
impl Command {
    pub fn id(&self) -> u8 {
        use Command::*;
        match self {
            LoadFromPc { .. } => 0x20,
            DumpToPc { .. } => 0x30,
            TargetSideFifo(_) => 0x40,
            SetSaveType(_) => 0x70,
            SetCicType(_) => 0x72,
            SetCiExtended(_) => 0x74,
            VersionRequest => 0x80,
        }
    }
    
    pub fn encode_packet(&self) -> Vec<u8> {
        let mut packet = BytesMut::from([self.id(), 0x43, 0x4D, 0x44].as_ref());
        
        use Command::*;
        match self {
            LoadFromPc { addr, bank_id_len, data } => {
                packet.put_u32(*addr);
                packet.put_u32(*bank_id_len);
                packet.put_slice(data);
            },
            DumpToPc { addr, bank_id_len } => {
                packet.put_u32(*addr);
                packet.put_u32(*bank_id_len);
            },
            TargetSideFifo(data) => packet.put_slice(data),
            SetSaveType(savetype) => packet.put_u32((savetype_index(*savetype).unwrap_or(0) as u32) & 0x0000000F),
            SetCicType(cic) => packet.put_u32((cic_index(*cic).unwrap_or(1) & 0x7) as u32 | 0x80000000),
            SetCiExtended(enable) => packet.put_u32(*enable),
            VersionRequest => (),
        }
        
        packet.to_vec()
    }
    
    pub fn recv_length(&self) -> u32 {
        use Command::*;
        match self {
            LoadFromPc { .. } => 0,
            DumpToPc { bank_id_len, .. } => bank_id_len & 0x00FFFFFF,
            TargetSideFifo(_) => 0,
            SetSaveType(_) => 0,
            SetCicType(_) => 0,
            SetCiExtended(_) => 0,
            VersionRequest => 8,
        }
    }
    
    /// Checks if provided `data` is a valid and complete command "footer" from the 64drive.
    /// 
    /// `data` must be exactly 4 bytes long.
    /// 
    /// # Example
    /// If the DUMP_TO_PC command was previously sent, `data` should be the 4 bytes recieved _after_
    /// the payload data requested from the cartridge: `[0x43, 0x4D, 0x50, 0x30]` (3 constant bytes,
    /// followed by the command's ID number.)
    pub fn complete_check<D: AsRef<[u8]>>(&self, data: D) -> Result<()> {
        let expected = [0x43, 0x4D, 0x50, self.id()];
        if expected == data.as_ref() {
            Ok(())
        } else {
            Err(CommunicationFailed(format!("64Drive: complete packet mismatch: {:02X?} vs expected {expected:02X?}", data.as_ref())))
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Segment {
    Rom,
    Sram256,
    Sram768,
    FlashRam,
    Eeprom4,
    Eeprom16,
}
impl Segment {
    pub fn max_length(self, cart: &mut SixtyFourDrive) -> u32 {
        use Segment::*;
        match self {
            Rom if !cart.is_hw1().unwrap_or(false) => 240 * 1024 * 1024,
            Rom => 64 * 1024 * 1024,
            Sram256 => 32 * 1024,
            Sram768 => 96 * 1024,
            FlashRam => 128 * 1024,
            Eeprom4 => 512,
            Eeprom16 => 2 * 1024,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Model {
    HW1, HW2,
}

#[derive(Debug)]
pub struct SixtyFourDrive {
    device: Ftdi,
}
impl Flashcart for SixtyFourDrive {
    fn upload_rom(&mut self, data: &[u8]) -> Result<()> {
        self.upload(Segment::Rom, 0, data)
    }

    fn download_rom(&mut self, length: u32) -> Result<Vec<u8>> {
        self.download(Segment::Rom, 0, length)
    }

    fn set_cic(&mut self, cic: Cic) -> Result<()> {
        let cic_index = (cic_index(cic).unwrap_or(1) & 0x7) as u32 | 0x80000000;
        
        self.send_packet(Command::SetCicType(cic))?;
        
        debug!("CIC is set {:#010X}", cic_index);
        Ok(())
    }

    fn set_savetype(&mut self, savetype: SaveType) -> Result<()> {
        let savetype_index = (savetype_index(savetype).unwrap_or(0) as u32) & 0x0000000F;
        
        self.send_packet(Command::SetSaveType(savetype))?;
        
        debug!("SaveType is set {:#010X}", savetype_index);
        Ok(())
    }

    fn recv_debug(&mut self) -> Result<DebugResponse> {
        let buf = self.ftdi_read(4)?;
        if buf != b"DMA@"{
            debug!("buf mismatch: {buf:02X?}");
            std::thread::sleep(Duration::from_millis(5));
            self.device.purge_rx()?;
            return Err(Error::CommunicationFailed(format!("64drive: debug packet mismatch: {buf:02X?} vs expected {:02X?}", b"DMA@")))
        }
        
        let [kind, length @ ..]: [u8; 4] = self.ftdi_read(4)?.try_into().unwrap();
        
        let kind = DataType::from(kind);
        let length = u32::from_be_bytes([0, length[0], length[1], length[2]]) as usize;
        
        let data = self.ftdi_read(length)?;
        
        let complete = self.ftdi_read(4)?;
        if complete != b"CMPH" {
            return Err(Error::CommunicationFailed(format!("64drive: complete packet mismatch: {complete:02X?} vs expected {:02X?}", b"CMPH")));
        }
        
        debug!("Received {kind:?} data: {data:02X?}");
        
        Ok((kind, data))
    }

    fn send_debug(&mut self) -> Result<()> {
        todo!()
    }

    fn info(&mut self) -> Result<DeviceInfo> {
        self.device.device_info().map_err(|err| err.into())
    }
}
impl SixtyFourDrive {
    pub fn new(mut device: Ftdi) -> Result<Self> {
        device.reset().unwrap_or_default();
        device.set_timeouts(Duration::from_secs(10), Duration::from_secs(10))?;
        
        device.set_bit_mode(0xFF, BitMode::Reset)?;
        device.set_bit_mode(0xFF, BitMode::SyncFifo)?;
        
        device.purge_all()?;
        
        Ok(Self {
            device,
        })
    }
    
    fn is_hw1(&mut self) -> Result<bool> {
        let info = self.info()?;
        
        Ok(match (info.vendor_id, info.product_id, info.description.as_str()) {
            (0x0403, 0x6010, "64drive USB device A") => true,
            _ => false
        })
    }
    
    pub fn upload(&mut self, segment: Segment, offset: u32, data: &[u8]) -> Result<()> {
        const SIZE: u32 = 0x800000;
        
        let chunks = (data.len() as f32 / SIZE as f32).ceil() as u32;
        let bank = bank_index(&segment, self.is_hw1()?, false); //TODO detect stadium 2
        
        let mut data_index = 0;
        for i in 0..chunks {
            let length = min(data.len() - data_index, SIZE as usize);
            
            let addr = offset + (i as u32 * SIZE);
            let bank_id_len = bank | (length as u32 & 0x00FFFFFF);
            
            let cmd = Command::LoadFromPc {
                addr,
                bank_id_len,
                data: data[data_index..(data_index + length)].to_vec(),
            };
            
            data_index += length;
            
            debug!("Uploading data. offset: {addr:#010X}, banklen: {bank_id_len:#010X}");
            self.send_packet(cmd)?;
            debug!("Write complete.");
        }
        
        debug!("Upload complete!");
        Ok(())
    }
    
    pub fn download(&mut self, segment: Segment, offset: u32, mut length: u32) -> Result<Vec<u8>> {
        const SIZE: u32 = 0x20000;
        
        if length == 0 {
            return Ok(vec![]);
        }
        
        if length & 3 > 0 {
            length = length + (4 - (length & 3));
        }
        length = min(length, segment.max_length(self));
        
        let chunks = (length as f32 / SIZE as f32).ceil() as u32;
        let bank = bank_index(&segment, self.is_hw1()?, false); //TODO detect stadium 2
        
        let mut data = vec![];
        let mut data_index = 0;
        for i in 0..chunks {
            let length = min(length - data_index, SIZE);
            if (length & 0x00FFFFFF) < 4 {
                break;
            }
            data_index += length;
            
            let addr = offset + (i * SIZE);
            let bank_id_len = bank | (length & 0x00FFFFFF);
            
            let cmd = Command::DumpToPc {
                addr,
                bank_id_len,
            };
            
            debug!("Downloading data. offset: {addr:#010X}, banklen: {bank_id_len:#010X}");
            let buf = self.send_packet(cmd)?;
            debug!("Read complete.");
            
            data.extend_from_slice(&buf);
        }
        
        debug!("Download complete! {:.4} MiB", data.len() as f32 / (1024.0 * 1024.0));
        Ok(data)
    }
    
    fn send_packet(&mut self, cmd: Command) -> Result<Vec<u8>> {
        self.ftdi_write(cmd.encode_packet())?;
        
        let response = self.ftdi_read(cmd.recv_length() as usize)?;
        cmd.complete_check(self.ftdi_read(4)?)?;
        
        Ok(response)
    }
    
    fn ftdi_read(&mut self, length: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0xFFu8; length];
        if length == 0 {
            return Ok(buf);
        }
        
        self.device.read_all(&mut buf)?;
        
        Ok(buf)
    }
    
    fn ftdi_write<D: AsRef<[u8]>>(&mut self, data: D) -> Result<()> {
        self.device.write_all(data.as_ref()).map_err(|err| err.into())
    }
}



fn bank_index(segment: &Segment, is_hw1: bool, is_stadium: bool) -> u32 {
    use Segment::*;
    
    (match segment {
        Rom => 1,
        Sram256 => 2,
        Sram768 => 3,
        FlashRam if is_hw1 && is_stadium => 5,
        FlashRam => 4,
        Eeprom4 => 6,
        Eeprom16 => 6,
    } << 24)
}

/// Gets the 64drive index value associated with each CIC variant.
/// 
/// `Cic::Auto` and `Cic::Unknown` will return `None`.
fn cic_index(cic: Cic) -> Option<u8> {
    use Cic::*;
    
    match cic {
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

fn savetype_index(savetype: SaveType) -> Option<u8> {
    use SaveType::*;
    
    match savetype {
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