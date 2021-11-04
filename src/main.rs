extern crate portmidi as pm;

use portmidi::{DeviceInfo, InputPort, MidiEvent, PortMidi, Result};
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    match &args[1..] {
        [input_name, output_name] => {
            match pm::PortMidi::new() {
                Ok(context) => {
                    let devices = devices(&context);

                    match select_devices(&devices, &input_name, &output_name) {
                        Some((input,  output)) => {
                            in_separator(|| {
                                println!("Selected MIDI devices:");
                                println!("{}", input);
                                println!("{}", output);
                            });

                            match DeviceInfo::new(input.id()) {
                                Ok(device) => {
                                    let event = listen_to_device(&context, device);
                                    println!("Event: {:?}", event);
                                },
                                _ => println!("Could not find MIDI device with id: {}", input.id()),
                            }
                        },
                        _ => println!("No devices matching both the input name and output name")
                    }
                },
                _ => println!("Could not initialize MIDI context"),
            }
        },
        _ => println!("Usage: ./midi-hub <input-name> <output-name>"),
    }
}

fn in_separator<F: Fn()>(f: F) {
    println!("====================");
    f();
    println!("====================");
}

fn devices(context: &PortMidi) -> Vec<DeviceInfo> {
    let devices: Result<Vec<DeviceInfo>> = context.devices();
    let unwrapped_devices = devices.unwrap_or_else(|err| {
        println!("Error: {}", err);
        return Vec::new();
    });

    in_separator(|| {
        if unwrapped_devices.len() == 0 {
            println!("No MIDI devices are connected!");
        } else {
            println!("MIDI devices:");
            for device in &unwrapped_devices {
                println!("{}", device);
            }
        }
    });

    return unwrapped_devices;
}

fn select_devices<'a>(devices: &'a Vec<DeviceInfo>, input_name: &'a str, output_name: &'a str) -> Option<(&'a DeviceInfo, &'a DeviceInfo)> {
    let mut selected_input_device = None;
    let mut selected_output_device = None;

    for device in devices {
        if selected_input_device.is_none() && device.is_input() && device.name() == input_name {
            selected_input_device = Some(device);
        } else if selected_output_device.is_none() && device.is_output() && device.name() == output_name {
            selected_output_device = Some(device);
        }
    }

    return match (selected_input_device, selected_output_device) {
        (Some(input), Some(output)) => Some((input, output)),
        _ => None,
    };
}

fn listen_to_device(context: &PortMidi, device: DeviceInfo) -> MidiEvent {
    let mut event: Option<MidiEvent> = None;
    let ref mut input_port: Option<InputPort> = context.input_port(device, 1024).ok();

    while event.is_none() {
        event = input_port.as_mut().and_then(|port| port.read().ok().flatten());
    }

    return event.unwrap();
}
