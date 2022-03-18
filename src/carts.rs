use std::fmt::{Display, Formatter};
use libftd2xx::{Ftdi, FtStatus, TimeoutError};


pub mod everdrive;
pub mod sixtyfourdrive;
pub mod summercart;


#[derive(Debug, PartialEq)]
pub enum Error {
    FtdiStatusError(FtStatus),
    FtdiTimeoutError(TimeoutError),
    ModelDetectFailed,
    UnsupportedOperation(String),
    CommandFailed(u8),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, PartialEq)]
pub enum Model {
    SixtyFourDriveHW1,
    SixtyFourDriveHW2,
    Everdrive3,
    EverdriveX7,
    SummerCart64,
    Other(String),
}

impl Display for Model {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use Model::*;
        match self {
            SixtyFourDriveHW1 => write!(f, "64drive HW1"),
            SixtyFourDriveHW2 => write!(f, "64drive HW2"),
            Everdrive3 => write!(f, "EverDrive v3"),
            EverdriveX7 => write!(f, "EverDrive X7"),
            SummerCart64 => write!(f, "SummerCart64"),
            Other(name) => write!(f, "{}", name),
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

pub trait Cartridge {
    fn new(device: Ftdi) -> Result<Box<dyn Cartridge>> where Self: Sized;
    fn upload(&mut self, segment: Segment, offset: u32, data: Vec<u8>) -> Result<()>;
    fn download(&mut self, segment: Segment, offset: u32) -> Result<Vec<u8>>;
    fn model(&mut self) -> Model;
    fn device(&mut self) -> &mut Ftdi;
}
