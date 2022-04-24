use std::collections::HashMap;
use std::iter::Extend;

use serde::{Serialize, Deserialize};

pub type Config = HashMap<String, DeviceConfig>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub device_type: DeviceType,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceType {
    Default,
    LaunchpadPro,
}

pub fn configure() -> Result<Config, Box<dyn std::error::Error>> {
    let mut config = Config::new();

    let mut name = String::new();
    let mut device_id = String::new();
    let mut add_device = String::new();

    println!("[midi] please enter the name of the MIDI device: ");
    std::io::stdin().read_line(&mut name)?;
    let name = name.trim().to_string();
    println!("");

    println!("[midi] please enter the identifier you want to give to this device: ");
    std::io::stdin().read_line(&mut device_id)?;
    let device_id = device_id.trim().to_string();
    println!("");

    let device_type = configure_type()?;

    config.insert(device_id, DeviceConfig {
        name,
        device_type,
    });

    println!("[midi] do you want to configure another device? (yes|no)");
    std::io::stdin().read_line(&mut add_device)?;
    let add_device = add_device.trim().to_string();
    println!("");

    if add_device == "yes" {
        config.extend(configure()?);
    }

    return Ok(config);
}

fn configure_type() -> Result<DeviceType, Box<dyn std::error::Error>> {
    let mut device_type = String::new();

    println!("[midi] please enter the type of the MIDI device (default|launchpadpro): ");
    std::io::stdin().read_line(&mut device_type)?;
    let device_type = device_type.trim().to_string();
    println!("");

    return match toml::from_str(device_type.as_str()) {
        Ok(device_type) => Ok(device_type),
        Err(err) => {
            eprintln!("[midi] could not parse the device type: {}", err);
            configure_type()
        },
    }
}
