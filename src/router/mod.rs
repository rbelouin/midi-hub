extern crate signal_hook as sh;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::apps;
use crate::midi;
use midi::{Connections, Error, Event, Reader, Writer, IntoAppIndex, FromImage, FromAppColors};
use midi::launchpadpro::{LaunchpadPro, LaunchpadProEvent};
use crate::server::HttpServer;

const MIDI_DEVICE_POLL_INTERVAL: Duration = Duration::from_millis(10_000);
const MIDI_EVENT_POLL_INTERVAL: Duration = Duration::from_millis(10);

enum AppName {
    Spotify,
    Youtube,
}

pub struct RunConfig {
    pub input_name: String,
    pub output_name: String,
    pub launchpad_name: String,
    pub spotify_config: apps::spotify::Config,
    pub youtube_config: apps::youtube::Config,
}

pub struct Router {
    config: RunConfig,
    term: Arc<AtomicBool>,
    server: HttpServer,
    spotify_app: apps::spotify::app::Spotify<LaunchpadProEvent>,
    youtube_app: apps::youtube::app::Youtube<LaunchpadProEvent>,
    selected_app: AppName,
}

impl Router {
    pub fn new(config: RunConfig) -> Self {
        let term = Arc::new(AtomicBool::new(false));

        let server = HttpServer::start();
        let spotify_app = apps::spotify::app::Spotify::new(config.spotify_config.clone());
        let youtube_app = apps::youtube::app::Youtube::new(config.youtube_config.clone());

        return Router {
            config,
            term,
            server,
            spotify_app,
            youtube_app,
            selected_app: AppName::Spotify,
        };
    }

    pub fn run(&mut self) -> Result<(), Error> {
        println!("Press ^C or send SIGINT to terminate the program");
        let _sigint = sh::flag::register(sh::consts::signal::SIGINT, Arc::clone(&self.term));

        let mut inner_result = Ok(());
        while !self.term.load(Ordering::Relaxed) && inner_result.is_ok() {
            inner_result = self.run_one_cycle(Instant::now());
        }
        return inner_result;
    }

    fn run_one_cycle(&mut self, start: Instant) -> Result<(), Error> {
        return Connections::new().and_then(|connections| {
            let mut input = connections.create_input_port(&self.config.input_name);
            let mut output = connections.create_output_port(&self.config.output_name);
            let mut launchpad = connections.create_bidirectional_ports(&self.config.launchpad_name)
                .map(|ports| LaunchpadPro::from(ports));

            let mut result = Ok(());

            while !self.term.load(Ordering::Relaxed) && result.is_ok() && start.elapsed() < MIDI_DEVICE_POLL_INTERVAL {
                let forward_result = match (input.as_mut(), output.as_mut()) {
                    (Ok(i), Ok(o)) => match i.read_midi() {
                        Ok(Some(e)) => {
                            println!("MIDI event: {:?}", e);
                            o.write(Event::Midi(e))
                        },
                        _ => Ok(()),
                    },
                    (Err(e), _) => Err(*e),
                    (_, Err(e)) => Err(*e),
                };

                let launchpad_result = match launchpad.as_mut() {
                    Ok(launchpad) => {
                        let _ = LaunchpadProEvent::from_app_colors(vec![
                            apps::spotify::app::COLOR,
                            apps::youtube::app::COLOR,
                        ]).and_then(|event| launchpad.write(event));

                        match self.selected_app {
                            AppName::Spotify => {
                                let event = self.spotify_app.receive();
                                match event {
                                    Ok(apps::spotify::Out::Command(command)) => {
                                        let _ = self.server.send(command);
                                    },
                                    Ok(apps::spotify::Out::Event(event)) => {
                                        let _ = launchpad.write(event);
                                    },
                                    _ => {},
                                }
                            },
                            AppName::Youtube => {
                                let command = self.youtube_app.receive();
                                match command {
                                    Ok(apps::youtube::Out::Command(command)) => {
                                        let _ = self.server.send(command);
                                    },
                                    Ok(apps::youtube::Out::Event(event)) => {
                                        let _ = launchpad.write(event);
                                    },
                                    _ => {},
                                }
                            },
                        }

                        match launchpad.read() {
                            Ok(Some(event)) => {
                                match event.clone().into_app_index() {
                                    Ok(Some(0)) => {
                                        println!("Selecting Spotify");
                                        self.selected_app = AppName::Spotify;
                                        let _ = LaunchpadProEvent::from_image(apps::spotify::app::get_spotify_logo())
                                            .and_then(|event| launchpad.write(event));
                                    },
                                    Ok(Some(1)) => {
                                        println!("Selecting Youtube");
                                        self.selected_app = AppName::Youtube;
                                        let _ = LaunchpadProEvent::from_image(apps::youtube::app::get_youtube_logo())
                                            .and_then(|event| launchpad.write(event));
                                    },
                                    _ => {
                                        match self.selected_app {
                                            AppName::Spotify => self.spotify_app.send(event),
                                            AppName::Youtube => self.youtube_app.send(event),
                                        }
                                    },
                                }
                                Ok(())
                            },
                            _ => Ok(()),
                        }
                    },
                    Err(e) => Err(*e),
                };

                result = forward_result.or(launchpad_result);
                match result {
                    Ok(_) => thread::sleep(MIDI_EVENT_POLL_INTERVAL),
                    _ => thread::sleep(MIDI_DEVICE_POLL_INTERVAL),
                }
            }

            return result;
        });
    }
}
