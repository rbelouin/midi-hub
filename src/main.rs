extern crate portmidi as pm;
extern crate signal_hook as sh;

use pm::{MidiEvent, MidiMessage};

use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

mod spotify;
mod image;
mod midi;
use midi::{Connections, Error, InputPort, OutputPort, Reader, Writer, ImageRenderer};
use midi::launchpadpro::LaunchpadPro;

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
    input: Result<InputPort<'a>, Error>,
    output: Result<OutputPort<'a>, Error>,
    spotify: Result<LaunchpadPro<'a>, Error>,
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
                return inner_result.map_err(|err| format!("{}", err));
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
) -> Result<(), Error> {
    return Connections::new().and_then(|connections| {
        let ports = select_ports(&connections, &config);
        let mut result = Ok(());
        match ports {
            Ports { input: Ok(mut input_port), output: Ok(mut output_port), spotify: _ } => {
                while !term.load(Ordering::Relaxed)
                    && result.is_ok()
                    && start.elapsed() < MIDI_DEVICE_POLL_INTERVAL
                {
                    result = forward_events(&mut input_port, &mut output_port);
                    thread::sleep(MIDI_EVENT_POLL_INTERVAL);
                }
            },
            Ports { input: _, output: _, spotify: Ok(mut ports) } => {
                while !term.load(Ordering::Relaxed)
                    && result.is_ok()
                    && start.elapsed() < MIDI_DEVICE_POLL_INTERVAL
                {
                    let cover_pixels = task_spawner.cover_pixels();
                    match cover_pixels {
                        Some(pixels) => {
                            let _ = ports.render(pixels);
                        },
                        None => {},
                    }
                    result = send_spotify_tasks(task_spawner, config.playlist_id.clone(), &mut ports);
                    thread::sleep(MIDI_EVENT_POLL_INTERVAL);
                }
            },
            _ => {
                println!("Could not find the configured ports");
                thread::sleep(MIDI_DEVICE_POLL_INTERVAL);
            },
        }
        return result;
    });
}

fn select_ports<'a, 'b, 'c>(
    connections: &'a Connections,
    config: &'b RunConfig,
) -> Ports<'a> {
    let input = connections.create_input_port(&config.input_name);
    let output = connections.create_output_port(&config.output_name);
    let spotify = connections.create_bidirectional_ports(&config.spotify_selector)
        .map(|ports| LaunchpadPro::from(ports));

    return Ports { input, output, spotify };
}

fn forward_events<R: Reader, W: Writer>(reader: &mut R, writer: &mut W) -> Result<(), Error> {
    return match reader.read() {
        Ok(Some(e)) => {
            println!("MIDI event: {:?}", e);
            return writer.write(&e);
        },
        _ => Ok(()),
    };
}

fn send_spotify_tasks<R: Reader>(task_spawner: &spotify::SpotifyTaskSpawner, playlist_id: String, spotify_reader: &mut R) -> Result<(), Error> {
    return match spotify_reader.read() {
        Ok(Some(MidiEvent { message: MidiMessage { status: 144, data1, data2, data3: _ }, timestamp: _ })) => {
            println!("MIDI event: {:?} {:?}", data1, data2);
            match map_to_old_index(data1).filter(|_index| data2 > 0) {
                Some(index) => task_spawner.spawn_task(spotify::SpotifyTask {
                    action: spotify::SpotifyAction::Play { index: index.into() },
                    playlist_id,
                }),
                None => {},
            }
            return Ok(());
        },
        _ => Ok(()),
    };
}

fn map_to_old_index(code: u8) -> Option<u8> {
    println!("code: {}", code);
    let mut grid = vec![
        vec![28, 29, 30, 31, 60, 61, 62, 63],
        vec![24, 25, 26, 27, 56, 57, 58, 59],
        vec![20, 21, 22, 23, 52, 53, 54, 55],
        vec![16, 17, 18, 19, 48, 49, 50, 51],
        vec![12, 13, 14, 15, 44, 45, 46, 47],
        vec![08, 09, 10, 11, 40, 41, 42, 43],
        vec![04, 05, 06, 07, 36, 37, 38, 39],
        vec![00, 01, 02, 03, 32, 33, 34, 35],
    ];

    grid.reverse();

    let row: usize = (code % 10).into();
    let column: usize = (code / 10).into();

    if row >= 1 && row <= 8 && column >= 1 && column <= 8 {
        return Some(grid[column - 1][row - 1]);
    } else {
        return None;
    }
}
