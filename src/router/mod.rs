extern crate signal_hook as sh;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::spotify;
use crate::midi;
use midi::{Connections, Error, InputPort, OutputPort, Reader, Writer, ImageRenderer, IndexReader};
use midi::launchpadpro::LaunchpadPro;

const MIDI_DEVICE_POLL_INTERVAL: Duration = Duration::from_millis(10_000);
const MIDI_EVENT_POLL_INTERVAL: Duration = Duration::from_millis(10);

pub struct RunConfig {
    pub spotify_app_config: spotify::SpotifyAppConfig,
    pub input_name: String,
    pub output_name: String,
    pub spotify_selector: String,
}

struct Ports<'a> {
    input: Result<InputPort<'a>, Error>,
    output: Result<OutputPort<'a>, Error>,
    spotify: Result<LaunchpadPro<'a>, Error>,
}

pub fn run(config: &RunConfig) -> Result<(), Error> {
    let ref term = Arc::new(AtomicBool::new(false));
    println!("Press ^C or send SIGINT to terminate the program");
    let _sigint = sh::flag::register(sh::consts::signal::SIGINT, Arc::clone(term));

    let task_spawner = spotify::SpotifyTaskSpawner::new(config.spotify_app_config.clone());
    let mut inner_result = Ok(());
    while !term.load(Ordering::Relaxed) && inner_result.is_ok() {
        inner_result = cycle(&config, &task_spawner, Instant::now(), term);
    }
    return inner_result;
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
                    let selected_covers = task_spawner.selected_covers();
                    match selected_covers {
                        Some(images) => {
                            let _ = ports.render(images);
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
