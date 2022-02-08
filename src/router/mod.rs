extern crate signal_hook as sh;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::spotify;
use crate::midi;
use midi::{Connections, Error, Writer, ImageRenderer, IndexReader};
use midi::launchpadpro::LaunchpadPro;

const MIDI_DEVICE_POLL_INTERVAL: Duration = Duration::from_millis(10_000);
const MIDI_EVENT_POLL_INTERVAL: Duration = Duration::from_millis(10);

pub struct RunConfig {
    pub spotify_app_config: spotify::SpotifyAppConfig,
    pub input_name: String,
    pub output_name: String,
    pub spotify_selector: String,
}

pub struct Router {
    config: RunConfig,
    term: Arc<AtomicBool>,
    spotify_spawner: spotify::SpotifyTaskSpawner,
}

impl Router {
    pub fn new(config: RunConfig) -> Self {
        let term = Arc::new(AtomicBool::new(false));

        let spotify_spawner = spotify::SpotifyTaskSpawner::new(config.spotify_app_config.clone());

        return Router {
            config,
            term,
            spotify_spawner,
        };
    }

    pub fn run(&self) -> Result<(), Error> {
        println!("Press ^C or send SIGINT to terminate the program");
        let _sigint = sh::flag::register(sh::consts::signal::SIGINT, Arc::clone(&self.term));

        let mut inner_result = Ok(());
        while !self.term.load(Ordering::Relaxed) && inner_result.is_ok() {
            inner_result = self.run_one_cycle(Instant::now());
        }
        return inner_result;
    }

    fn run_one_cycle(&self, start: Instant) -> Result<(), Error> {
        return Connections::new().and_then(|connections| {
            let mut input = connections.create_input_port(&self.config.input_name);
            let mut output = connections.create_output_port(&self.config.output_name);
            let mut spotify = connections.create_bidirectional_ports(&self.config.spotify_selector)
                .map(|ports| LaunchpadPro::from(ports));

            let mut result = Ok(());

            while !self.term.load(Ordering::Relaxed) && result.is_ok() && start.elapsed() < MIDI_DEVICE_POLL_INTERVAL {
                let forward_result = match (input.as_mut(), output.as_mut()) {
                    (Ok(i), Ok(o)) => match i.read() {
                        Ok(Some(e)) => {
                            println!("MIDI event: {:?}", e);
                            o.write(&e)
                        },
                        _ => Ok(()),
                    },
                    (Err(e), _) => Err(*e),
                    (_, Err(e)) => Err(*e),
                };

                let spotify_result = match spotify.as_mut() {
                    Ok(spotify) => {
                        let selected_covers = self.spotify_spawner.selected_covers();
                        match selected_covers {
                            Some(images) => {
                                let _ = spotify.render(images);
                            },
                            None => {},
                        }

                        match spotify.read_index() {
                            Ok(Some(index)) => {
                                self.spotify_spawner.spawn_task(spotify::SpotifyTask::Play { index: index.into()  });
                                Ok(())
                            },
                            _ => Ok(()),
                        }
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
}
