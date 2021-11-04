extern crate portmidi as pm;

use portmidi::{DeviceInfo, MidiEvent, PortMidi};

use std::env;
use std::thread;
use std::time::Duration;

const BUFFER_SIZE: usize = 1024;

fn main() {
    let result = args().and_then(|(input_name, output_name)| {
        return context().and_then(|context| {
            return select_devices(&context, &input_name, &output_name).and_then(|(input_device, output_device)| {
                return read_from_device(&context, input_device).and_then(|event| {
                    return write_to_device(&context, output_device, event);
                });
            });
        });
    });

    match result {
        Ok(_) => println!("Completed successfully. Bye!"),
        Err(err) => println!("{}", err),
    }
}

fn args() -> Result<(String, String), String> {
    let args: Vec<String> = env::args().collect();
    return match &args[1..] {
        [input_name, output_name] => Ok((String::from(input_name), String::from(output_name))),
        _ => Err(String::from("Usage: ./midi-hub <input-name> <output-name>")),
    }
}

fn context() -> Result<PortMidi, String> {
    return pm::PortMidi::new().map_err(|err| format!("Could not initialize MIDI context: {}", err));
}

fn select_devices(context: &PortMidi, input_name: &String, output_name: &String) -> Result<(DeviceInfo, DeviceInfo), String> {
    return context.devices().map_err(|err| format!("Error when retrieving the MIDI devices: {}", err)).and_then(|devices| {
        if devices.len() == 0 {
            return Err(String::from("No MIDI devices are connected."));
        }

        let mut selected_input_device = Err(format!("Could not find an input device with the name {}", input_name));
        let mut selected_output_device = Err(format!("Could not find an output device with the name {}", output_name));

        println!("MIDI devices:");
        for device in devices {
            println!("{}", device);
            if selected_input_device.is_err() && device.is_input() && device.name() == input_name {
                selected_input_device = Ok(device);
            } else if selected_output_device.is_err() && device.is_output() && device.name() == output_name {
                selected_output_device = Ok(device);
            }
        }

        return selected_input_device.and_then(|input| {
            return selected_output_device.map(|output| (input, output));
        });
    });
}

fn read_from_device(context: &PortMidi, device: DeviceInfo) -> Result<MidiEvent, String> {
    let duration = Duration::from_millis(10);

    println!("Waiting for a MIDI event to be emitted by {}", device.name());
    return context.input_port(device, BUFFER_SIZE).as_mut().map_err(|err| format!("Error when retrieving the input port: {}", err)).and_then(|port| {
        let mut event: Result<MidiEvent, String> = Err(String::from("No MIDI event has been emitted"));
        while event.is_err() {
            match port.read() {
                Ok(Some(e)) => event = Ok(e),
                _ => {},
            }
            thread::sleep(duration);
        }
        return event;
    });
}

fn write_to_device(context: &PortMidi, device: DeviceInfo, event: MidiEvent) -> Result<(), String> {
    return context.output_port(device, BUFFER_SIZE).as_mut().map_err(|err| format!("Error when retrieving the output port: {}", err)).and_then(|port| {
        return port.write_event(event).map_err(|err| format!("Error when writing the event: {}", err));
    });
}
