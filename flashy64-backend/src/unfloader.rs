use num_enum::{FromPrimitive, IntoPrimitive};

#[derive(Debug, Copy, Clone, PartialEq, Eq, IntoPrimitive, FromPrimitive)]
#[repr(u8)]
pub enum DataType {
    Text = 0x01,
    RawBinary = 0x02,
    Header = 0x03,
    Screenshot = 0x04,
    
    #[num_enum(default)]
    Unknown,
}

pub type DebugResponse = (DataType, Vec<u8>);