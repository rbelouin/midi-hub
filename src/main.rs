#[macro_use] extern crate async_trait;
#[macro_use] extern crate mockall;

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

enum Command {
    INIT,
    RUN,
}

fn main() {
    let result = get_command().and_then(|command| match command {
        Command::INIT => router::configure().map_err(|err| format!("{}", err))
            .and_then(|config| toml::to_string(&config).map_err(|err| format!("{}", err)))
            .map(|config| {
                println!("You can copy/paste the following to your config.toml:\n");
                println!("{}", config)
            }),
        Command::RUN => read_config().and_then(|config| {
            let mut router = router::Router::new(config);
            router.run().map_err(|err| format!("{}", err))
        }),
    });

    match result {
        Ok(_) => println!("Completed successfully. Bye!"),
        Err(err) => println!("{}", err),
    }
}

fn get_command() -> Result<Command, String> {
    let args = env::args().collect::<Vec<String>>();
    let command = args.get(1).filter(|_| args.len() == 2);
    return match command.map(|s| s.as_str()) {
        Some("init") => Ok(Command::INIT),
        Some("run") => Ok(Command::RUN),
        _ => Err(String::from("Usage: ./midi-hub [init|run]")),
    }
}

fn read_config() -> Result<router::Config, String> {
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
