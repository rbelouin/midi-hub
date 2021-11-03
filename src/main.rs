extern crate portmidi as pm;

use portmidi::{DeviceInfo, PortMidi, Result};

fn main() {
    let devices = devices();

    if devices.len() == 0 {
        println!("No devices!");
    } else {
        for device in devices {
            println!("Device: {}", device);
        }
    }
}

fn devices() -> Vec<DeviceInfo> {
    let context: Result<PortMidi> = pm::PortMidi::new();
    let devices: Result<Vec<DeviceInfo>> = context.and_then(|c| c.devices());

    return devices.unwrap_or_else(|err| {
        println!("Error: {}", err);
        return Vec::new();
    });
}
