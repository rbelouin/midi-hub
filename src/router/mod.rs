extern crate signal_hook as sh;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::spotify;
use crate::midi;
use midi::{Connections, Error, Reader, Writer, ImageRenderer, IndexReader};
use midi::launchpadpro::LaunchpadPro;

const MIDI_DEVICE_POLL_INTERVAL: Duration = Duration::from_millis(10_000);
const MIDI_EVENT_POLL_INTERVAL: Duration = Duration::from_millis(10);

pub struct RunConfig {
    pub spotify_app_config: spotify::SpotifyAppConfig,
    pub input_name: String,
    pub output_name: String,
    pub spotify_selector: String,
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
        let mut input = connections.create_input_port(&config.input_name);
        let mut output = connections.create_output_port(&config.output_name);
        let mut spotify = connections.create_bidirectional_ports(&config.spotify_selector)
            .map(|ports| LaunchpadPro::from(ports));

        let mut result = Ok(());

        while !term.load(Ordering::Relaxed) && result.is_ok() && start.elapsed() < MIDI_DEVICE_POLL_INTERVAL {
            let forward_result = match (input.as_mut(), output.as_mut()) {
                (Ok(i), Ok(o)) => forward_events(i, o),
                (Err(e), _) => Err(*e),
                (_, Err(e)) => Err(*e),
            };

            let spotify_result = match spotify.as_mut() {
                Ok(spotify) => {
                    let selected_covers = task_spawner.selected_covers();
                    match selected_covers {
                        Some(images) => {
                            let _ = spotify.render(images);
                        },
                        None => {},
                    }
                    send_spotify_tasks(task_spawner, spotify)
                },
                Err(e) => Err(*e),
            };

            result = forward_result.or(spotify_result);
            match result {
                Ok(_) => thread::sleep(MIDI_EVENT_POLL_INTERVAL),
                _ => thread::sleep(MIDI_DEVICE_POLL_INTERVAL),
            }
        }

        return result;
    });
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
