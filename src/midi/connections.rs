use std::collections::HashMap;

extern crate portmidi;
use portmidi::{DeviceInfo, Direction, PortMidi};
pub use portmidi::{InputPort, OutputPort};

use super::error::Error;

/// The buffer size is quite arbitrary
const BUFFER_SIZE: usize = 1024;

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

    pub fn create_input_port(&self, name: &String) -> Result<InputPort, Error> {
        println!("[midi] initializing input {}", name);
        let device = self.input_devices.get(name).ok_or(Error::DeviceNotFound)?;
        return self.context.input_port(device.clone(), BUFFER_SIZE).map_err(|err| {
            eprintln!("[midi] error when initializing input {}: {}", name, err);
            Error::PortInitializationError
        });
    }

    pub fn create_output_port(&self, name: &String) -> Result<OutputPort, Error> {
        println!("[midi] initializing output {}", name);
        let device = self.output_devices.get(name).ok_or(Error::DeviceNotFound)?;
        return self.context.output_port(device.clone(), BUFFER_SIZE).map_err(|err| {
            eprintln!("[midi] error when initializing output {}: {}", name, err);
            Error::PortInitializationError
        });
    }

    pub fn create_bidirectional_ports(&self, name: &String) -> Result<(InputPort, OutputPort), Error> {
        let input_port = self.create_input_port(name)?;
        let output_port = self.create_output_port(name)?;
        return Ok((input_port, output_port));
    }

    pub fn get_device_names(&self) -> Vec<String> {
        let input_device_names = self.input_devices.keys().collect::<Vec<&String>>();
        let output_device_names = self.output_devices.keys().collect::<Vec<&String>>();

        let mut device_names = vec![input_device_names, output_device_names].concat()
            .into_iter()
            .map(|name| name.clone())
            .collect::<Vec<String>>();

        device_names.sort();
        device_names.dedup();
        return device_names;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(not(feature = "launchpadpro"))]
    #[cfg(not(feature = "planckez"))]
    fn new_should_return_ok() {
        use super::*;

        let connections = Connections::new();
        assert!(connections.is_ok(), "Connections::new() did return an error");
    }

    #[test]
    #[cfg(feature = "planckez")]
    fn connections_should_match_expectations() {
        use std::thread;
        use std::time::Duration;
        use portmidi::{MidiEvent, MidiMessage};
        use super::*;

        let connections = Connections::new();
        assert!(connections.is_ok(), "Connections::new() did return an error");

        let name = "Planck EZ".to_string();
        let result = connections.as_ref().unwrap().create_bidirectional_ports(&name);
        assert!(result.is_ok(), "{:?} should have been found as a tuple of input/output ports", name);

        if let Ok((input_port, mut output_port)) = result {
            let input_device = input_port.device();
            let output_device = output_port.device();
            assert!(input_device.is_input(), "`{:?}` should be an input device", input_device);
            assert!(output_device.is_output(), "`{:?}` should be an output device", output_device);

            // You should here a Ab5 for 300ms
            let _ = output_port.write_event(MidiEvent::from(MidiMessage::from([144, 80, 36, 0])));
            thread::sleep(Duration::from_millis(300));
            let _ = output_port.write_event(MidiEvent::from(MidiMessage::from([128, 80, 0, 0])));
        }
    }
}
