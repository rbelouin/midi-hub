extern crate portmidi as pm;
extern crate signal_hook as sh;

use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

mod spotify;
mod image;
mod midi;
use midi::{Connections, Error, InputPort, OutputPort, Reader, Writer, ImageRenderer, IndexReader};
use midi::launchpadpro::LaunchpadPro;

const MIDI_DEVICE_POLL_INTERVAL: Duration = Duration::from_millis(10_000);
const MIDI_EVENT_POLL_INTERVAL: Duration = Duration::from_millis(10);

enum Config {
    LoginConfig {
        config: spotify::SpotifyAppConfig,
    },
    RunConfig {
        config: RunConfig,
    },
}

struct RunConfig {
    spotify_app_config: spotify::SpotifyAppConfig,
    input_name: String,
    output_name: String,
    spotify_selector: String,
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
                    config: spotify::SpotifyAppConfig {
                        authorization: spotify::authorization::SpotifyAuthorizationConfig {
                            client_id: String::from(client_id),
                            client_secret: String::from(client_secret),
                            refresh_token: None,
                        },
                        // this is not needed to generate a refresh token
                        playlist_id: "".to_string(),
                    },
                }),
                _ => Err(String::from("Usage: ./midi-hub login <client-id> <client-secret>")),
            };
        },
        Some("run") => {
            return match &args[2..] {
                [client_id, client_secret, input_name, output_name, spotify_selector, playlist_id, token] => Ok(Config::RunConfig {
                    config: RunConfig {
                        spotify_app_config: spotify::SpotifyAppConfig {
                            authorization: spotify::authorization::SpotifyAuthorizationConfig {
                                client_id: String::from(client_id),
                                client_secret: String::from(client_secret),
                                refresh_token: Some(String::from(token)),
                            },
                            playlist_id: String::from(playlist_id),
                        },
                        input_name: String::from(input_name),
                        output_name: String::from(output_name),
                        spotify_selector: String::from(spotify_selector),
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
                    result = send_spotify_tasks(task_spawner, &mut ports);
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

fn send_spotify_tasks<IR: IndexReader>(task_spawner: &spotify::SpotifyTaskSpawner, spotify_reader: &mut IR) -> Result<(), Error> {
    return match spotify_reader.read_index() {
        Ok(Some(index)) => {
            task_spawner.spawn_task(spotify::SpotifyTask::Play { index: index.into() });
            return Ok(());
        },
        _ => Ok(()),
    };
}
