extern crate portmidi as pm;

use portmidi::{DeviceInfo, PortMidi, Result};
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    match &args[1..] {
        [input_name, output_name] => {
            let devices = devices();

            match select_devices(&devices, &input_name, &output_name) {
                Some((input,  output)) => {
                    in_separator(|| {
                        println!("Selected MIDI devices:");
                        println!("{}", input);
                        println!("{}", output);
                    });
                },
                _ => println!("No devices matching both the input name and output name")
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

fn devices() -> Vec<DeviceInfo> {
    let context: Result<PortMidi> = pm::PortMidi::new();
    let devices: Result<Vec<DeviceInfo>> = context.and_then(|c| c.devices());
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
