use std::sync::Arc;
use std::collections::HashMap;

use crate::midi::{Error, Connections, InputPort, OutputPort};
use crate::midi::features::Features;

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

    pub fn get_input_port<'a>(&self, id: &str, connections: &'a Connections) -> Result<DeviceWithInputPort<'a>, Error> {
        let device = self.get(id).ok_or(Error::DeviceNotFound)?;
        let port = device.get_input_port(connections)?;
        Ok(DeviceWithInputPort {
            id: device.id.clone(),
            name: device.name.clone(),
            device_type: device.device_type.clone(),
            features: Arc::clone(&device.features),
            port,
        })
    }

    pub fn get_output_port<'a>(&self, id: &str, connections: &'a Connections) -> Result<DeviceWithOutputPort<'a>, Error> {
        let device = self.get(id).ok_or(Error::DeviceNotFound)?;
        let port = device.get_output_port(connections)?;
        Ok(DeviceWithOutputPort {
            id: device.id.clone(),
            name: device.name.clone(),
            device_type: device.device_type.clone(),
            features: Arc::clone(&device.features),
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
                features: match device_config.device_type {
                    config::DeviceType::Default => Arc::new(default::DefaultFeatures::new()),
                    config::DeviceType::LaunchpadPro => Arc::new(launchpadpro::LaunchpadProFeatures::new()),
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
    pub features: Arc<dyn Features + Sync + Send>,
}

impl Device {
    pub fn get_input_port<'a>(&self, connections: &'a Connections) -> Result<InputPort<'a>, Error> {
        return connections.create_input_port(&self.name);
    }

    pub fn get_output_port<'a>(&self, connections: &'a Connections) -> Result<OutputPort<'a>, Error> {
        return connections.create_output_port(&self.name);
    }
}

pub struct DeviceWithInputPort<'a> {
    pub id: String,
    pub name: String,
    pub device_type: config::DeviceType,
    pub features: Arc<dyn Features + Sync + Send>,
    pub port: InputPort<'a>,
}

pub struct DeviceWithOutputPort<'a> {
    pub id: String,
    pub name: String,
    pub device_type: config::DeviceType,
    pub features: Arc<dyn Features + Sync + Send>,
    pub port: OutputPort<'a>,
}
