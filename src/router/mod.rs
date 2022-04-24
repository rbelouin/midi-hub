extern crate signal_hook as sh;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use serde::{Serialize, Deserialize};

use crate::apps;
use crate::apps::{App, Out};
use crate::midi;
use midi::{Connections, Error, Event, Reader, Writer, IntoAppIndex, FromImage, FromAppColors};
use midi::launchpadpro::{LaunchpadPro, LaunchpadProEvent};
use crate::server::HttpServer;

const MIDI_DEVICE_POLL_INTERVAL: Duration = Duration::from_millis(10_000);
const MIDI_EVENT_POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Serialize, Deserialize)]
pub struct RunConfig {
    pub input_name: String,
    pub output_name: String,
    pub launchpad_name: String,
    pub forward: apps::forward::config::Config,
    pub spotify: apps::spotify::config::Config,
    pub youtube: apps::youtube::config::Config,
}

pub struct Router {
    config: RunConfig,
    term: Arc<AtomicBool>,
    server: HttpServer,
    forward_app: Box<dyn App<Event, Out<Event>>>,
    apps: Vec<Box<dyn App<LaunchpadProEvent, Out<LaunchpadProEvent>>>>,
    selected_app: usize,
}

impl Router {
    pub fn new(config: RunConfig) -> Self {
        let term = Arc::new(AtomicBool::new(false));

        let server = HttpServer::start();
        let forward_app = apps::forward::app::Forward::new(config.forward.clone());
        let spotify_app = apps::spotify::app::Spotify::new(config.spotify.clone());
        let youtube_app = apps::youtube::app::Youtube::new(config.youtube.clone());

        return Router {
            config,
            term,
            server,
            // The forward app is not an app you can access via app selection yet
            forward_app: Box::new(forward_app),
            apps: vec![Box::new(spotify_app), Box::new(youtube_app)],
            selected_app: 0,
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
                let input_result = match input.as_mut() {
                    Ok(input) => {
                        match Reader::read(input) {
                            Ok(Some(event)) => self.forward_app.send(event),
                            _  => Ok(()),
                        }
                    },
                    _ => Ok(()),
                };

                let output_result = match output.as_mut() {
                    Ok(output) => {
                        let event = self.forward_app.receive();
                        match event {
                            Ok(Out::Server(command)) => {
                                self.server.send(command);
                                Ok(())
                            },
                            Ok(Out::Event(event)) => {
                                output.write(event)
                            },
                            _ => Ok(()),
                        }
                    },
                    _ => Ok(()),
                };

                let launchpad_result = match launchpad.as_mut() {
                    Ok(launchpad) => {
                        let _ = LaunchpadProEvent::from_app_colors(
                            self.apps.iter().map(|app| app.get_color()).collect()
                        ).and_then(|event| launchpad.write(event));

                        if self.apps.len() > self.selected_app {
                            let event = self.apps[self.selected_app].receive();
                            match event {
                                Ok(Out::Server(command)) => {
                                    let _ = self.server.send(command);
                                },
                                Ok(Out::Event(event)) => {
                                    let _ = launchpad.write(event);
                                },
                                _ => {},
                            }
                        }

                        match launchpad.read() {
                            Ok(Some(event)) => {
                                let selected_app = event.clone().into_app_index().ok().flatten()
                                    .and_then(|app_index| {
                                        let selected_app = self.apps.get(app_index as usize);
                                        if selected_app.is_some() {
                                            self.selected_app = app_index as usize;
                                        }
                                        return selected_app;
                                    });

                                match selected_app {
                                    Some(selected_app) => {
                                        println!("Selecting {}", selected_app.get_name());
                                        let _ = LaunchpadProEvent::from_image(selected_app.get_logo())
                                            .and_then(|event| launchpad.write(event));
                                    },
                                    _ => {
                                        match self.apps.get(self.selected_app) {
                                            Some(app) => app.send(event)
                                                .unwrap_or_else(|err| eprintln!("[{}] could not send event: {:?}", app.get_name(), err)),
                                            None => eprintln!("No app found for index: {}", self.selected_app),
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

                result = input_result.or(output_result).or(launchpad_result);
                match result {
                    Ok(_) => thread::sleep(MIDI_EVENT_POLL_INTERVAL),
                    _ => thread::sleep(MIDI_DEVICE_POLL_INTERVAL),
                }
            }

            return result;
        });
    }
}
