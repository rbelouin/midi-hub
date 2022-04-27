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
    pub apps: apps::Config,
}

pub struct Router {
    term: Arc<AtomicBool>,
    server: HttpServer,
    devices: Devices,
    forward_app: Box<dyn App>,
    selection_app: Box<dyn App>,
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
            config.apps.forward.expect("forward should be defined"),
            input.transformer,
            output.transformer,
        );

        let selection_app = apps::selection::app::Selection::new(
            config.apps.selection.expect("selection should be defined"),
            launchpad.transformer,
            launchpad.transformer,
        );

        return Router {
            term,
            server,
            devices,
            forward_app: Box::new(forward_app),
            selection_app: Box::new(selection_app),
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
                        let event = self.selection_app.receive();
                        match event {
                            Ok(Out::Server(command)) => {
                                let _ = self.server.send(command);
                            },
                            Ok(Out::Midi(event)) => {
                                let _ = launchpad.port.write(event);
                            },
                            _ => {},
                        }

                        match launchpad.port.read() {
                            Ok(Some(event)) => self.selection_app.send(event)
                                .map_err(|err| {
                                    eprintln!("[router] could not send event to the selection app: {}", err);
                                    Error::WriteError
                                }),
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
