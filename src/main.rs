extern crate portmidi as pm;
extern crate signal_hook as sh;

use std::env;
use std::fs;
use std::path::PathBuf;
use toml::value::Value;

mod apps;
mod image;
mod midi;
mod router;
mod server;

enum Config {
    LoginConfig {
        config: apps::spotify::authorization::Config,
    },
    RunConfig {
        config: router::RunConfig,
    },
}

fn main() {
    let result = args().and_then(|config| {
        match config {
            Config::LoginConfig { config } => {
                return apps::spotify::authorization::login_sync(config.clone()).and_then(|token| token.refresh_token.ok_or(()))
                    .map(|refresh_token| {
                        println!("Please use this refresh token to start the service: {:?}", refresh_token);
                        return ();
                    })
                    .map_err(|()| String::from("Could not log in"));
            },
            Config::RunConfig { config } => {
                let mut router = router::Router::new(config);
                router.run().map_err(|err| format!("{}", err))
            }
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
                    config: apps::spotify::authorization::Config {
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
                [] => read_config().map(|config| Config::RunConfig { config }),
                _ => Err(String::from("Usage: ./midi-hub run")),
            };
        },
        _ => Err(String::from("Usage ./midi-hub [login|run] <args>")),
    };
}

fn read_config() -> Result<router::RunConfig, String> {
    let mut config_file = std::env::var("XDG_CONFIG_HOME").map(|xdg_config_home| PathBuf::from(xdg_config_home))
        .or_else(|_| std::env::var("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|_| PathBuf::from("."));

    config_file.push("midi-hub");
    config_file.push("config.toml");

    let content = fs::read_to_string(config_file.clone())
        .map_err(|err| format!("Could not find config.toml in {:?}: {:?}", config_file, err))?;
    let config = content.parse::<Value>()
        .and_then(|toml_value| toml_value.try_into())
        .map_err(|err| format!("Could not parse config.toml: {:?}", err))?;
    return Ok(config);
}
