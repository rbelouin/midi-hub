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
mod image;
mod launchpad;

const BUFFER_SIZE: usize = 1024;
const MIDI_DEVICE_POLL_INTERVAL: Duration = Duration::from_millis(10_000);
const MIDI_EVENT_POLL_INTERVAL: Duration = Duration::from_millis(10);

enum Config {
    LoginConfig {
        config: spotify::authorization::SpotifyAppConfig,
    },
    RunConfig {
        config: RunConfig,
    },
}

struct RunConfig {
    spotify_app_config: spotify::authorization::SpotifyAppConfig,
    input_name: String,
    output_name: String,
    spotify_selector: String,
    playlist_id: String,
}

struct Ports<'a> {
    input: Result<InputPort<'a>, String>,
    output: Result<OutputPort<'a>, String>,
    spotify_input: Result<InputPort<'a>, String>,
    spotify_output: Result<OutputPort<'a>, String>,
}

fn main() {
    let result = args().and_then(|config| {
        match config {
            Config::LoginConfig { config } => {
                let task_spawner = spotify::SpotifyTaskSpawner::new(config.clone());
                return task_spawner.login_sync().and_then(|token| token.refresh_token.ok_or(()))
                    .map(|refresh_token| {
                        println!("Please use this refresh token to start the service: {:?}", refresh_token);
                        return ();
                    })
                    .map_err(|()| String::from("Could not log in"));
            },
            Config::RunConfig { config } => {
                let ref term = Arc::new(AtomicBool::new(false));
                println!("Press ^C or send SIGINT to terminate the program");
                let _sigint = sh::flag::register(sh::consts::signal::SIGINT, Arc::clone(term));

                let task_spawner = spotify::SpotifyTaskSpawner::new(config.spotify_app_config.clone());
                let mut inner_result = Ok(());
                while !term.load(Ordering::Relaxed) && inner_result.is_ok() {
                    inner_result = cycle(&config, &task_spawner, Instant::now(), term);
                }
                return inner_result;
            },
        }
    });

    match result {
        Ok(_) => println!("Completed successfully. Bye!"),
        Err(err) => println!("{}", err),
    }
}

fn args() -> Result<Config, String> {
    let args: Vec<String> = env::args().collect();
    return match args.get(1).map(|s| s.as_str()) {
        Some("login") => {
            return match &args[2..] {
                [client_id, client_secret] => Ok(Config::LoginConfig {
                    config: spotify::authorization::SpotifyAppConfig {
                        client_id: String::from(client_id),
                        client_secret: String::from(client_secret),
                        refresh_token: None,
                    },
                }),
                _ => Err(String::from("Usage: ./midi-hub login <client-id> <client-secret>")),
            };
        },
        Some("run") => {
            return match &args[2..] {
                [client_id, client_secret, input_name, output_name, spotify_selector, playlist_id, token] => Ok(Config::RunConfig {
                    config: RunConfig {
                        spotify_app_config: spotify::authorization::SpotifyAppConfig {
                            client_id: String::from(client_id),
                            client_secret: String::from(client_secret),
                            refresh_token: Some(String::from(token)),
                        },
                        input_name: String::from(input_name),
                        output_name: String::from(output_name),
                        spotify_selector: String::from(spotify_selector),
                        playlist_id: String::from(playlist_id),
                    },
                }),
                _ => Err(String::from("Usage: ./midi-hub run <client-id> <client-secret> <input-name> <output-name> <spotify-selector> <playlist-id> <spotify-token>")),
            };
        },
        _ => Err(String::from("Usage ./midi-hub [login|run] <args>")),
    };
}

fn cycle(
    config: &RunConfig,
    task_spawner: &spotify::SpotifyTaskSpawner,
    start: Instant,
    term: &Arc<AtomicBool>,
) -> Result<(), String> {
    return context().and_then(|context| {
        let ports = select_ports(&context, &config);
        let mut result = Ok(());
        match ports {
            Ok(Ports { input: Ok(mut input_port), output: Ok(mut output_port), spotify_input: _ , spotify_output: _ }) => {
                while !term.load(Ordering::Relaxed)
                    && result.is_ok()
                    && start.elapsed() < MIDI_DEVICE_POLL_INTERVAL
                {
                    result = forward_events(&mut input_port, &mut output_port);
                    thread::sleep(MIDI_EVENT_POLL_INTERVAL);
                }
            },
            Ok(Ports { input: _, output: _, spotify_input: Ok(mut s1), spotify_output: Ok(s2) }) => {
                while !term.load(Ordering::Relaxed)
                    && result.is_ok()
                    && start.elapsed() < MIDI_DEVICE_POLL_INTERVAL
                {
                    let cover_pixels = task_spawner.cover_pixels();
                    match cover_pixels {
                        Some(pixels) => launchpad::render_pixels(&s2, pixels),
                        None => {},
                    }
                    result = send_spotify_tasks(task_spawner, config.playlist_id.clone(), &mut s1);
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
    config: &'b RunConfig,
) -> Result<Ports<'a>, String> {
    return devices(context).map(|devices| {
        let mut ports = Ports {
            input: Err(format!("Could not find input device: {}", config.input_name)),
            output: Err(format!("Could not find output device: {}", config.output_name)),
            spotify_input: Err(format!("Could not find spotify device: {}", config.spotify_selector)),
            spotify_output: Err(format!("Could not find spotify device: {}", config.spotify_selector)),
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
            } else if ports.spotify_input.is_err() && device.is_input() && device.name().to_string() == config.spotify_selector {
                ports.spotify_input = context.input_port(device, BUFFER_SIZE).map_err(|err| {
                    return format!("Could not retrieve input port ({}): {}", config.spotify_selector, err);
                });
            } else if ports.spotify_output.is_err() && device.is_output() && device.name().to_string() == config.spotify_selector {
                ports.spotify_output = context.output_port(device, BUFFER_SIZE).map_err(|err| {
                    return format!("Could not retrieve output port ({}): {}", config.spotify_selector, err);
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

fn send_spotify_tasks(task_spawner: &spotify::SpotifyTaskSpawner, playlist_id: String, spotify_port: &mut InputPort) -> Result<(), String> {
    return match spotify_port.read() {
        Ok(Some(MidiEvent { message: MidiMessage { status: 144, data1, data2, data3: _ }, timestamp: _ })) => {
            if data1 >= 36 && data1 < 100 && data2 > 0 {
                println!("MIDI event: {:?} {:?}", data1, data2);
                task_spawner.spawn_task(spotify::SpotifyTask {
                    action: spotify::SpotifyAction::Play { index: (data1 - 36).into() },
                    playlist_id,
                });
            }
            return Ok(());
        },
        _ => Ok(()),
    };
}
