use std::collections::HashMap;

use serde::{Serialize, Deserialize};

use dialoguer::{theme::ColorfulTheme, Input, MultiSelect, Select};

use crate::midi::Connections;

pub type Config = HashMap<String, DeviceConfig>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub device_type: DeviceType,
}

#[derive(Clone, Debug, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceType {
    Default,
    LaunchpadPro,
}

pub fn configure() -> Result<Config, Box<dyn std::error::Error>> {
    let mut config = Config::new();

    let connections = Connections::new()?;
    let device_names = connections.get_device_names();

    if device_names.is_empty() {
        panic!("[midi] no devices found. Have you connected your MIDI devices before proceeding?");
    }

    let mut selected_items = vec![];

    while selected_items.is_empty() {
        selected_items = MultiSelect::with_theme(&ColorfulTheme::default())
            .with_prompt("[midi] please select the MIDI devices you want to configure (use spacebar to select):")
            .items(device_names.as_slice())
            .interact()?;
    }

    for selected_item in selected_items {
        let name = device_names[selected_item].trim().to_string();
        let device_id: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("[midi] please enter the identifier you want to give to this device: ")
            .interact_text()?;

        let device_id = device_id.trim().to_string();
        let device_type = configure_type(&name)?;

        config.insert(device_id, DeviceConfig {
            name,
            device_type,
        });
    }

    return Ok(config);
}

fn configure_type(name: &String) -> Result<DeviceType, Box<dyn std::error::Error>> {
    let device_types = vec![DeviceType::Default, DeviceType::LaunchpadPro];
    let serialized_device_types = device_types.as_slice().into_iter()
        .map(|t| format!("{:?}", t))
        .collect::<Vec<String>>();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("[midi] please select what the type of the device \"{}\" is (use spacebar to select):", name))
        .items(serialized_device_types.as_slice())
        .interact()?;

    return Ok(device_types[selection]);
}
