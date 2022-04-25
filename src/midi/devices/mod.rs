use std::collections::HashMap;

use crate::midi::{Error, EventTransformer, Connections, InputPort, OutputPort};

pub mod config;

// device types
pub mod default;
pub mod launchpadpro;

pub struct Devices {
    devices: HashMap<String, Device>,
}

impl Devices {
    pub fn get(&self, id: &str) -> Option<&Device> {
        return self.devices.get(id);
    }

    pub fn get_port<'a>(&self, id: &str, connections: &'a Connections) -> Result<DeviceWithPort<'a>, Error> {
        let device = self.get(id).ok_or(Error::DeviceNotFound)?;
        let port = device.get_port(connections)?;
        Ok(DeviceWithPort {
            id: device.id.clone(),
            name: device.name.clone(),
            device_type: device.device_type.clone(),
            transformer: device.transformer,
            port,
        })
    }
}

impl From<&config::Config> for Devices {
    fn from(config: &config::Config) -> Devices {
        let mut devices = HashMap::new();

        for (device_id, device_config) in config {
            devices.insert(device_id.clone(), Device {
                id: device_id.to_string(),
                name: device_config.name.to_string(),
                device_type: device_config.device_type.clone(),
                transformer: match device_config.device_type {
                    config::DeviceType::Default => default::transformer(),
                    config::DeviceType::LaunchpadPro => launchpadpro::transformer(),
                },
            });
        }

        return Devices { devices };
    }
}

pub struct Device {
    pub id: String,
    pub name: String,
    pub device_type: config::DeviceType,
    pub transformer: &'static (dyn EventTransformer + Sync),
}

impl Device {
    pub fn get_port<'a>(&self, connections: &'a Connections) -> Result<(InputPort<'a>, OutputPort<'a>), Error> {
        return connections.create_bidirectional_ports(&self.name);
    }
}

pub struct DeviceWithPort<'a> {
    pub id: String,
    pub name: String,
    pub device_type: config::DeviceType,
    pub transformer: &'static (dyn EventTransformer + Sync),
    pub port: (InputPort<'a>, OutputPort<'a>),
}
