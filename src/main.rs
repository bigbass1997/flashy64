
use clap::{AppSettings, Arg, Command};
use libftd2xx::{Ftdi, FtdiCommon};
use flashy64::cart;
use flashy64::carts::Segment;

fn main() {
    let matches = Command::new("flashy64")
        .arg(Arg::new("upload")
            .short('u')
            .long("upload")
            .takes_value(true)
            .help("Upload rom at the provided path."))
        .arg(Arg::new("list")
            .short('l')
            .long("list")
            .help("List available devices."))
        .arg(Arg::new("device")
            .short('d')
            .long("device")
            .takes_value(true)
            .help("Specify the device to use, by its serial number."))
        .next_line_help(true)
        //.arg_required_else_help(true)
        .setting(AppSettings::DeriveDisplayOrder)
        .get_matches();
    
    if matches.is_present("list") {
        if let Ok(carts) = flashy64::list_carts() {
            for mut cart in carts {
                let info = cart.device().device_info().unwrap();
                println!("{} | {}",
                    info.serial_number,
                    cart.model()
                );
            }
        }
        
        return;
    }
    
    let dev_result = match matches.value_of("device") {
        Some(serial) => Ftdi::with_serial_number(serial), 
        None => Ftdi::new()
    };
    let mut device = match dev_result {
        Ok(device) => device,
        Err(err) => panic!("Error: {}", err)
    };
    
    if let Some(path) = matches.value_of("upload") {
        let data = std::fs::read(path).unwrap();
        let mut cart = cart(device).unwrap();
        cart.upload(Segment::Rom, 0, data).unwrap();
        
        cart.device().close().unwrap();
        return;
    }
    
    
    device.write_all(&[0x80, 0x43, 0x4D, 0x44]).unwrap();
    let mut buf = [0u8; 12];
    device.read_all(&mut buf).unwrap();
    
    println!("{:04X}, {}, {}, {}",
        u16::from_be_bytes([buf[0], buf[1]]),
        u16::from_be_bytes([buf[2], buf[3]]),
        String::from_utf8_lossy(&buf[4..=7]).to_string(),
        buf[8..] == [0x43, 0x4D, 0x50, 0x80]
    );
}