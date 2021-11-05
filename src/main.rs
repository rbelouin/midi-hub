extern crate portmidi as pm;
extern crate signal_hook as sh;

use portmidi::{DeviceInfo, InputPort, OutputPort, PortMidi};

use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const BUFFER_SIZE: usize = 1024;
const MIDI_DEVICE_POLL_INTERVAL: Duration = Duration::from_millis(10_000);
const MIDI_EVENT_POLL_INTERVAL: Duration = Duration::from_millis(10);

fn main() {
    let ref term = Arc::new(AtomicBool::new(false));

    println!("Press ^C or send SIGINT to terminate the program");
    let _sigint = sh::flag::register(sh::consts::signal::SIGINT, Arc::clone(term));

    let result = args().and_then(|(input_name, output_name)| {
        let mut inner_result = Ok(());
        while !term.load(Ordering::Relaxed) && inner_result.is_ok() {
            inner_result = cycle(&input_name, &output_name, Instant::now(), term);
        }
        return inner_result;
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
    };
}

fn cycle(
    input_name: &String,
    output_name: &String,
    start: Instant,
    term: &Arc<AtomicBool>,
) -> Result<(), String> {
    return context().and_then(|context| {
        let ports = select_ports(&context, &input_name, &output_name);
        let mut result = Ok(());
        match ports {
            Ok((mut input_port, mut output_port)) => {
                while !term.load(Ordering::Relaxed)
                    && result.is_ok()
                    && start.elapsed() < MIDI_DEVICE_POLL_INTERVAL
                {
                    result = forward_events(&mut input_port, &mut output_port);
                    thread::sleep(MIDI_EVENT_POLL_INTERVAL);
                }
            },
            Err(err) => {
                println!("{}", err);
                thread::sleep(MIDI_DEVICE_POLL_INTERVAL);
            },
        }
        return result;
    });
}

fn context() -> Result<PortMidi, String> {
    return pm::PortMidi::new()
        .map_err(|err| format!("Could not initialize MIDI context: {}", err));
}

fn select_ports<'a, 'b, 'c>(
    context: &'a PortMidi,
    input_name: &'b String,
    output_name: &'c String,
) -> Result<(InputPort<'a>, OutputPort<'a>), String> {
    return devices(context).and_then(|devices| {
        return select_devices(devices, input_name, output_name).and_then(
            |(input_device, output_device)| {
                return context
                    .input_port(input_device, BUFFER_SIZE)
                    .map_err(|err| {
                        format!(
                            "Could not retrieve the input port for device ({}): {}",
                            input_name, err
                        )
                    })
                    .and_then(|input_port| {
                        return context
                            .output_port(output_device, BUFFER_SIZE)
                            .map_err(|err| {
                                format!(
                                    "Could not retrieve the output port for device({}): {}",
                                    output_name, err
                                )
                            })
                            .map(|output_port| {
                                return (input_port, output_port);
                            });
                    });
            },
        );
    });
}

fn select_devices(
    devices: Vec<DeviceInfo>,
    input_name: &String,
    output_name: &String,
) -> Result<(DeviceInfo, DeviceInfo), String> {
    let mut selected_input_device = Err(format!(
        "Could not find an input device with the name {}",
        input_name
    ));
    let mut selected_output_device = Err(format!(
        "Could not find an output device with the name {}",
        output_name
    ));

    for device in devices {
        if selected_input_device.is_err() && device.is_input() && device.name() == input_name {
            selected_input_device = Ok(device);
        } else if selected_output_device.is_err()
            && device.is_output()
            && device.name() == output_name
        {
            selected_output_device = Ok(device);
        }
    }

    return selected_input_device.and_then(|input| {
        return selected_output_device.map(|output| (input, output));
    });
}

fn devices(context: &PortMidi) -> Result<Vec<DeviceInfo>, String> {
    return context
        .devices()
        .map_err(|err| format!("Error when retrieving MIDI devices: {}", err))
        .and_then(|devices| {
            if devices.len() == 0 {
                return Err(String::from("No MIDI devices are connected."));
            }

            println!("MIDI devices:");
            for device in &devices {
                println!("{}", device);
            }

            return Ok(devices);
        });
}

fn forward_events(input_port: &mut InputPort, output_port: &mut OutputPort) -> Result<(), String> {
    return match input_port.read() {
        Ok(Some(e)) => output_port
            .write_event(e)
            .map_err(|err| format!("Error when writing the event: {}", err)),
        _ => Ok(()),
    };
}
