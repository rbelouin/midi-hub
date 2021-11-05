extern crate portmidi as pm;
extern crate signal_hook as sh;

use pm::{MidiEvent, MidiMessage};
use portmidi::{DeviceInfo, InputPort, OutputPort, PortMidi};

use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

mod spotify;

const BUFFER_SIZE: usize = 1024;
const MIDI_DEVICE_POLL_INTERVAL: Duration = Duration::from_millis(10_000);
const MIDI_EVENT_POLL_INTERVAL: Duration = Duration::from_millis(10);

struct Config {
    input_name: String,
    output_name: String,
    spotify_selector: String,
    playlist_id: String,
    token: String,
}

struct Ports<'a> {
    input: Result<InputPort<'a>, String>,
    output: Result<OutputPort<'a>, String>,
    spotify: Result<InputPort<'a>, String>,
}

fn main() {
    let ref term = Arc::new(AtomicBool::new(false));
    let task_spawner = spotify::SpotifyTaskSpawner::new();

    println!("Press ^C or send SIGINT to terminate the program");
    let _sigint = sh::flag::register(sh::consts::signal::SIGINT, Arc::clone(term));

    let result = args().and_then(|config| {
        let mut inner_result = Ok(());
        while !term.load(Ordering::Relaxed) && inner_result.is_ok() {
            inner_result = cycle(&config, &task_spawner, Instant::now(), term);
        }
        return inner_result;
    });

    match result {
        Ok(_) => println!("Completed successfully. Bye!"),
        Err(err) => println!("{}", err),
    }
}

fn args() -> Result<Config, String> {
    let args: Vec<String> = env::args().collect();
    return match &args[1..] {
        [input_name, output_name, spotify_selector, playlist_id, token] => Ok(Config {
            input_name: String::from(input_name),
            output_name: String::from(output_name),
            spotify_selector: String::from(spotify_selector),
            playlist_id: String::from(playlist_id),
            token: String::from(token),
        }),
        _ => Err(String::from("Usage: ./midi-hub <input-name> <output-name> <spotify-selector> <playlist-id> <spotify-token>")),
    };
}

fn cycle(
    config: &Config,
    task_spawner: &spotify::SpotifyTaskSpawner,
    start: Instant,
    term: &Arc<AtomicBool>,
) -> Result<(), String> {
    return context().and_then(|context| {
        let ports = select_ports(&context, &config);
        let mut result = Ok(());
        match ports {
            Ok(Ports { input: Ok(mut input_port), output: Ok(mut output_port), spotify: _ }) => {
                while !term.load(Ordering::Relaxed)
                    && result.is_ok()
                    && start.elapsed() < MIDI_DEVICE_POLL_INTERVAL
                {
                    result = forward_events(&mut input_port, &mut output_port);
                    thread::sleep(MIDI_EVENT_POLL_INTERVAL);
                }
            },
            Ok(Ports { input: _, output: _, spotify: Ok(mut s) }) => {
                while !term.load(Ordering::Relaxed)
                    && result.is_ok()
                    && start.elapsed() < MIDI_DEVICE_POLL_INTERVAL
                {
                    result = send_spotify_tasks(task_spawner, config.playlist_id.clone(), config.token.clone(), &mut s);
                    thread::sleep(MIDI_EVENT_POLL_INTERVAL);
                }
            },
            Err(err) => {
                println!("{}", err);
                thread::sleep(MIDI_DEVICE_POLL_INTERVAL);
            },
            _ => {
                println!("Other kind of error");
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
    config: &'b Config,
) -> Result<Ports<'a>, String> {
    return devices(context).map(|devices| {
        let mut ports = Ports {
            input: Err(format!("Could not find input device: {}", config.input_name)),
            output: Err(format!("Could not find output device: {}", config.output_name)),
            spotify: Err(format!("Could not find spotify device: {}", config.spotify_selector)),
        };

        for device in devices {
            if ports.input.is_err() && device.is_input() && device.name().to_string() == config.input_name {
                ports.input = context.input_port(device, BUFFER_SIZE).map_err(|err| {
                    return format!("Could not retrieve input port ({}): {}", config.input_name, err);
                });
            } else if ports.output.is_err() && device.is_output() && device.name().to_string() == config.output_name {
                ports.output = context.output_port(device, BUFFER_SIZE).map_err(|err| {
                    return format!("Could not retrieve output port ({}): {}", config.output_name, err);
                });
            } else if ports.spotify.is_err() && device.is_input() && device.name().to_string() == config.spotify_selector {
                ports.spotify = context.input_port(device, BUFFER_SIZE).map_err(|err| {
                    return format!("Could not retrieve input port ({}): {}", config.spotify_selector, err);
                });
            }
        }

        return ports;
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
        Ok(Some(e)) => {
            println!("MIDI event: {:?}", e);
            return output_port
                .write_event(e)
                .map_err(|err| format!("Error when writing the event: {}", err));
        },
        _ => Ok(()),
    };
}

fn send_spotify_tasks(task_spawner: &spotify::SpotifyTaskSpawner, playlist_id: String, token: String, spotify_port: &mut InputPort) -> Result<(), String> {
    return match spotify_port.read() {
        Ok(Some(MidiEvent { message: MidiMessage { status: 144, data1, data2, data3: _ }, timestamp: _ })) => {
            if data1 >= 84 && data1 < 100 && data2 > 0 {
                println!("MIDI event: {:?} {:?}", data1, data2);
                task_spawner.spawn_task(spotify::SpotifyTask {
                    action: spotify::SpotifyAction::Play { index: (data1 - 84).into() },
                    token,
                    playlist_id,
                });
            }
            return Ok(());
        },
        _ => Ok(()),
    };
}
