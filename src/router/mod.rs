extern crate signal_hook as sh;

use std::convert::From;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use serde::{Serialize, Deserialize};

use crate::apps;
use crate::apps::{App, Out};
use crate::midi;
use midi::{Connections, Error, Reader, Writer, Devices};
use crate::server::HttpServer;

const MIDI_DEVICE_POLL_INTERVAL: Duration = Duration::from_millis(10_000);
const MIDI_EVENT_POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Serialize, Deserialize)]
pub struct RunConfig {
    pub devices: midi::devices::config::Config,
    pub forward: apps::forward::config::Config,
    pub spotify: apps::spotify::config::Config,
    pub youtube: apps::youtube::config::Config,
}

pub struct Router {
    term: Arc<AtomicBool>,
    server: HttpServer,
    devices: Devices,
    forward_app: Box<dyn App>,
    selection_app: apps::selection::Selection,
}

impl Router {
    pub fn new(config: RunConfig) -> Self {
        let term = Arc::new(AtomicBool::new(false));

        let server = HttpServer::start();

        let devices = Devices::from(&config.devices);
        let input = devices.get("input").expect("input device should be defined");
        let output = devices.get("output").expect("output device should be defined");
        let launchpad = devices.get("launchpad").expect("launchpad device should be defined");

        let forward_app = apps::forward::app::Forward::new(
            config.forward.clone(),
            input.transformer,
            output.transformer,
        );

        let spotify_app = apps::spotify::app::Spotify::new(
            config.spotify.clone(),
            launchpad.transformer,
            launchpad.transformer,
        );

        let youtube_app = apps::youtube::app::Youtube::new(
            config.youtube.clone(),
            launchpad.transformer,
            launchpad.transformer,
        );

        return Router {
            term,
            server,
            devices,
            // The forward app is not an app you can access via app selection yet
            forward_app: Box::new(forward_app),
            selection_app: apps::selection::Selection {
                apps: vec![Box::new(spotify_app), Box::new(youtube_app)],
                selected_app: 0,
            },
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
            let mut input = self.devices.get_port("input", &connections);
            let mut output = self.devices.get_port("output", &connections);
            let mut launchpad = self.devices.get_port("launchpad", &connections);

            let mut result = Ok(());

            while !self.term.load(Ordering::Relaxed) && result.is_ok() && start.elapsed() < MIDI_DEVICE_POLL_INTERVAL {
                let input_result = match input.as_mut() {
                    Ok(input) => {
                        match Reader::read(&mut input.port) {
                            Ok(Some(event)) => self.forward_app.send(event),
                            _  => Ok(()),
                        }
                    },
                    _ => {
                        Ok(())
                    },
                };

                let output_result = match output.as_mut() {
                    Ok(output) => {
                        let event = self.forward_app.receive();
                        match event {
                            Ok(Out::Server(command)) => {
                                self.server.send(command);
                                Ok(())
                            },
                            Ok(Out::Midi(event)) => {
                                output.port.write(event)
                            },
                            _ => Ok(()),
                        }
                    },
                    _ => {
                        Ok(())
                    }
                };

                let launchpad_result = match launchpad.as_mut() {
                    Ok(launchpad) => {
                        let _ = launchpad.transformer.from_app_colors(
                            self.selection_app.apps.iter().map(|app| app.get_color()).collect()
                        ).and_then(|event| launchpad.port.write(event));

                        if self.selection_app.apps.len() > self.selection_app.selected_app {
                            let event = self.selection_app.apps[self.selection_app.selected_app].receive();
                            match event {
                                Ok(Out::Server(command)) => {
                                    let _ = self.server.send(command);
                                },
                                Ok(Out::Midi(event)) => {
                                    let _ = launchpad.port.write(event);
                                },
                                _ => {},
                            }
                        }

                        match launchpad.port.read() {
                            Ok(Some(event)) => {
                                let selected_app = launchpad.transformer.into_app_index(event.clone()).ok().flatten()
                                    .and_then(|app_index| {
                                        let selected_app = self.selection_app.apps.get(app_index as usize);
                                        if selected_app.is_some() {
                                            self.selection_app.selected_app = app_index as usize;
                                        }
                                        return selected_app;
                                    });

                                match selected_app {
                                    Some(selected_app) => {
                                        println!("Selecting {}", selected_app.get_name());
                                        let _ = launchpad.transformer.from_image(selected_app.get_logo())
                                            .and_then(|event| launchpad.port.write(event));
                                    },
                                    _ => {
                                        match self.selection_app.apps.get(self.selection_app.selected_app) {
                                            Some(app) => app.send(event)
                                                .unwrap_or_else(|err| eprintln!("[{}] could not send event: {:?}", app.get_name(), err)),
                                            None => eprintln!("No app found for index: {}", self.selection_app.selected_app),
                                        }
                                    },
                                }
                                Ok(())
                            },
                            _ => Ok(()),
                        }
                    },
                    Err(e) => {
                        eprintln!("Error with launchpad: {}", e);
                        Err(*e)
                    },
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
