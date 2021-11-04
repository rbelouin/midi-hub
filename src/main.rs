extern crate portmidi as pm;
extern crate signal_hook as sh;

use portmidi::{DeviceInfo, PortMidi};

use std::env;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

const BUFFER_SIZE: usize = 1024;

fn main() {
    let term = Arc::new(AtomicBool::new(false));

    println!("Press ^C or send SIGINT to terminate the program");
    let _sigint = sh::flag::register(sh::consts::signal::SIGINT, Arc::clone(&term));

    let result = args().and_then(|(input_name, output_name)| {
        return context().and_then(|context| {
            return select_devices(&context, &input_name, &output_name).and_then(|(input_device, output_device)| {
                return connect_devices(&context, input_device, output_device, term);
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

fn connect_devices(context: &PortMidi, input_device: DeviceInfo, output_device: DeviceInfo, term: Arc<AtomicBool>) -> Result<(), String> {
    let duration = Duration::from_millis(10);

    println!("Waiting for MIDI events to be emitted by {}…", input_device.name());
    return context.input_port(input_device, BUFFER_SIZE).as_mut().map_err(|err| format!("Error when retrieving the input port: {}", err)).and_then(|input_port| {
        return context.output_port(output_device, BUFFER_SIZE).as_mut().map_err(|err| format!("Error when retrieving the output port: {}", err)).and_then(|output_port| {
            let mut result: Result<(), String> = Ok(());
            while !term.load(Ordering::Relaxed) && result.is_ok() {
                match input_port.read() {
                    Ok(Some(e)) => result = output_port.write_event(e).map_err(|err| format!("Error when writing the event: {}", err)),
                    _ => {},
                }
                thread::sleep(duration);
            }
            return result;
        });
    });
}
