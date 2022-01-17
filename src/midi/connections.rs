use std::collections::HashMap;

extern crate portmidi;
use portmidi::{DeviceInfo, Direction, PortMidi};

use super::error::Error;

/// This structure manages all MIDI connections
///
/// On macOS, hot-reload does not work and you will have to restart the program after plugging or
/// unplugging a device.
///
/// On linux, dropping the instance of PortMidi and re-instantiating one should be enough to
/// reflect the changes.
pub struct Connections {
    /// PortMidi’s base structure
    /// Keep a precious reference of it, as the input and output ports will have the same lifetime
    context: PortMidi,

    /// Input devices
    /// These are the MIDI devices you can read MIDI events from
    input_devices: HashMap<String, DeviceInfo>,

    /// Output devices
    /// These are the MIDI devices you can write MIDI events (or SysEx messages) to
    output_devices: HashMap<String, DeviceInfo>,
}

impl Connections {
    pub fn new() -> Result<Connections, Error> {
        let mut connections = PortMidi::new()
            .map(|context| Connections {
                context,
                input_devices: HashMap::new(),
                output_devices: HashMap::new(),
            })
            .map_err(|_| Error::ConnectionInitializationError)?;

        connections.load_devices()?;

        return Ok(connections);
    }

    fn load_devices(&mut self) -> Result<(), Error> {
        let devices = self.context.devices().map_err(|_| Error::DeviceLoadingError)?;
        for device in devices {
            let name = device.name().to_string();
            match device.direction() {
                Direction::Input => {
                    println!("[midi] registering {} as an input device", name);
                    self.input_devices.insert(name, device);
                },
                Direction::Output =>  {
                    println!("[midi] registering {} as an output device", name);
                    self.output_devices.insert(name, device);
                },
            }
        }
        return Ok(());
    }

    /// TODO remove this getter once ports are correctly exposed
    pub fn context(&self) -> &PortMidi {
        return &self.context;
    }

    /// TODO remove this getter once ports are correctly exposed
    pub fn get_input_device(&self, name: &String) -> Option<&DeviceInfo> {
        return self.input_devices.get(name);
    }

    /// TODO remove this getter once ports are correctly exposed
    pub fn get_output_device(&self, name: &String) -> Option<&DeviceInfo> {
        return self.output_devices.get(name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// CoreMIDI will crash if we instantiate PortMidi several times, and given that I am going to
    /// run these tests on macOS most of the time, let’s write some kind of integration tests that
    /// expect my Planck EZ keyboard (which supports MIDI) to be connected.
    fn connections_should_match_expectations() {
        let connections = Connections::new();
        assert!(connections.is_ok(), "`connections` should be an instance of Ok()");

        let name = "Planck EZ".to_string();
        let input_device = connections.as_ref().unwrap().get_input_device(&name);
        assert!(input_device.is_some(), "`{}` should have been found as an input device", name);
        assert!(input_device.unwrap().is_input(), "`{}` should be an input device", name);

        let output_device = connections.as_ref().unwrap().get_output_device(&name);
        assert!(output_device.is_some(), "`{}` should have been found as an output device", name);
        assert!(output_device.unwrap().is_output(), "`{}` should be an output device", name);
    }
}
