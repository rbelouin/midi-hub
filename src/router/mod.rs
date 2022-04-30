extern crate signal_hook as sh;

use std::collections::HashMap;
use std::convert::From;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use serde::{Serialize, Deserialize};
use tokio::sync::mpsc::error::TryRecvError;

use crate::apps;
use crate::apps::{App, Out};
use crate::midi;
use midi::{Connections, Error, Reader, Writer, Devices};
use crate::server::HttpServer;

const MIDI_DEVICE_POLL_INTERVAL: Duration = Duration::from_millis(10_000);
const MIDI_EVENT_POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub devices: midi::devices::config::Config,
    pub apps: apps::Config,
    pub links: Links,
}

pub type Links = HashMap<String, (String, String)>;

pub struct Router {
    term: Arc<AtomicBool>,
    server: HttpServer,
    devices: Devices,
    links: Vec<(Box<dyn App>, String, String)>,
}

impl Router {
    pub fn new(config: Config) -> Self {
        let term = Arc::new(AtomicBool::new(false));

        let server = HttpServer::start();

        let devices = Devices::from(&config.devices);
        let mut links = vec![];

        for (app_name, (input_name, output_name)) in &config.links {
            let input = devices.get(input_name.as_str())
                .expect(format!("{} is set as an input device for {}, but needs to be configured", input_name, app_name).as_str());

            let output = devices.get(output_name.as_str())
                .expect(format!("{} is set as an output device for {}, but needs to be configured", output_name, app_name).as_str());

            let app = config.apps.start(app_name, input.transformer, output.transformer)
                .expect(format!("The {} application needs to be configured", app_name).as_str());

            links.push((app, input_name.clone(), output_name.clone()));
        }

        return Router {
            term,
            server,
            devices,
            links,
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
            let mut resolved_links = vec![];

            for (app, input_name, output_name) in &mut self.links {
                let input = self.devices.get_input_port(input_name.as_str(), &connections);
                let output = self.devices.get_output_port(output_name.as_str(), &connections);
                resolved_links.push((app, input, output));
            }

            let mut execution = Ok(());

            while !self.term.load(Ordering::Relaxed) && execution.is_ok() && start.elapsed() < MIDI_DEVICE_POLL_INTERVAL {
                // If no application could read from/write to any devices, we’ll fail the execution
                // so that devices get pulled again.
                execution = Err(Error::DeviceNotFound);

                let server_command = match self.server.receive() {
                    Ok(command) => Some(command),
                    Err(TryRecvError::Disconnected) => {
                        eprintln!("[router] server has disconnected");
                        None
                    },
                    _ => None,
                };

                for (app, input, output) in &mut resolved_links {
                    let input_execution = match input.as_mut() {
                        Ok(input) => {
                            if let Some(command) = server_command.clone() {
                                app.send(command.into()).unwrap_or_else(|err| {
                                    eprintln!("[router] could not send event to app {}: {}", app.get_name(), err);
                                });
                            }

                            match Reader::read(&mut input.port) {
                                Ok(Some(event)) => app.send(event.into()).unwrap_or_else(|err| {
                                    eprintln!("[router] could not send event to app {}: {}", app.get_name(), err);
                                }),
                                Err(err) => eprintln!("[router] error when reading event from device {}: {}", input.id, err),
                                _ => {},
                            }
                            Ok(())
                        },
                        Err(err) => Err(*err),
                    };

                    let output_execution = match output.as_mut() {
                        Ok(output) => {
                            match app.receive() {
                                Ok(Out::Server(command)) => {
                                    self.server.send(command);
                                },
                                Ok(Out::Midi(event)) => output.port.write(event).unwrap_or_else(|err| {
                                    eprintln!("[router] error when writing event to device {}: {}", output.id, err);
                                }),
                                Err(TryRecvError::Disconnected) => {
                                    eprintln!("[router] app has disconnected: {}", app.get_name());
                                },
                                _ => {},
                            }
                            Ok(())
                        },
                        Err(err) => Err(*err),
                    };

                    execution = execution.or(input_execution.and(output_execution));
                }

                match execution {
                    Ok(_) => thread::sleep(MIDI_EVENT_POLL_INTERVAL),
                    _ => thread::sleep(MIDI_DEVICE_POLL_INTERVAL),
                }
            }

            return execution;
        });
    }
}

pub fn configure() -> Result<Config, Box<dyn std::error::Error>> {
    let devices = midi::devices::config::configure()?;
    let apps = apps::configure()?;

    let app_names = apps.get_configured_app_names();
    let links = configure_links(app_names)?;

    return Ok(Config {
        devices,
        apps,
        links,
    });
}

fn configure_links(app_names: Vec<String>) -> Result<HashMap<String, (String, String)>, Box<dyn std::error::Error>> {
    let mut links = HashMap::new();

    for app_name in app_names {
        let mut input_name = String::new();
        let mut output_name = String::new();

        println!("[router] what device do you want to use as an input for this app: {}?", app_name);
        std::io::stdin().read_line(&mut input_name)?;
        let input_name = input_name.trim().to_string();
        println!("");

        println!("[router] what device do you want to use as an output for this app: {}?", app_name);
        std::io::stdin().read_line(&mut output_name)?;
        let output_name = output_name.trim().to_string();
        println!("");

        links.insert(app_name, (input_name, output_name));
    }

    return Ok(links);
}
