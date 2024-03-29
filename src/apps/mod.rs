use std::convert::From;
use std::sync::Arc;

use serde::{Serialize, Deserialize};
use tokio::sync::mpsc::error::{SendError, TryRecvError};

use dialoguer::{theme::ColorfulTheme, Select};

use crate::image::Image;
pub use crate::midi::Event as MidiEvent;
pub use crate::midi::features::Features;
pub use crate::server::Command as ServerCommand;

pub mod forward;
pub mod paint;
pub mod selection;
pub mod spotify;
pub mod youtube;

pub trait App {
    /// Exposing a name enables the router to log more meaningful information
    fn get_name(&self) -> &'static str;

    /// Color will be used by devices who can assign a color to "app selection" buttons
    fn get_color(&self) -> [u8; 3];

    /// Logo will be used by devices who can render a picture when the application is selected
    fn get_logo(&self) -> Image;

    /// Send an event to be handled by the application
    fn send(&mut self, event: In) -> Result<(), SendError<In>>;

    /// Poll events emitted by the application
    fn receive(&mut self) -> Result<Out, TryRecvError>;

    /// Lifecycle callback that gets called every time the app gets the focus
    fn on_select(&mut self);
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub forward: Option<forward::config::Config>,
    pub paint: Option<paint::config::Config>,
    pub spotify: Option<spotify::config::Config>,
    pub youtube: Option<youtube::config::Config>,
    pub selection: Option<selection::config::Config>,
}

impl Config {
    pub fn start(
        &self,
        app_name: &str,
        input_features: Arc<dyn Features + Sync + Send>,
        output_features: Arc<dyn Features + Sync + Send>,
    ) -> Option<Box<dyn App>> {
        return match app_name {
            forward::app::NAME => {
                let config = self.forward.as_ref()?;
                Some(Box::new(forward::app::Forward::new(config.clone(), input_features, output_features)))
            }
            paint::app::NAME => {
                let config = self.paint.as_ref()?;
                Some(Box::new(paint::app::Paint::new(config.clone(), input_features, output_features)))
            },
            spotify::app::NAME => {
                let config = self.spotify.as_ref()?;
                Some(Box::new(spotify::app::Spotify::new(
                    config.clone(),
                    Box::new(spotify::client::SpotifyApiClientImpl::new()),
                    input_features,
                    output_features)))
            }
            youtube::app::NAME => {
                let config = self.youtube.as_ref()?;
                Some(Box::new(youtube::app::Youtube::new(config.clone(), input_features, output_features)))
            }
            selection::app::NAME => {
                let config = self.selection.as_ref()?;
                Some(Box::new(selection::app::Selection::new(config.clone(), input_features, output_features)))
            }
            _ => {
                eprintln!("[apps] unknown application: {}", app_name);
                None
            },
        }
    }

    pub fn start_all(
        &self,
        input_features: Arc<dyn Features + Sync + Send>,
        output_features: Arc<dyn Features + Sync + Send>,
    ) -> Vec<Box<dyn App>> {
        return self.get_configured_app_names().iter().flat_map(|name| {
            self.start(name.as_str(), Arc::clone(&input_features), Arc::clone(&output_features))
        }).collect();
    }

    pub fn get_configured_app_names(&self) -> Vec<String> {
        let toml_config = toml::Value::try_from(&self);
        let app_config = match toml_config {
            Ok(toml::Value::Table(table)) => table,
            _ => toml::map::Map::new(),
        };

        return app_config.keys().map(|key| key.to_string()).collect::<Vec<String>>();
    }
}

pub fn configure() -> Result<Config, Box<dyn std::error::Error>> {
    return Ok(Config {
        forward: configure_app(forward::app::NAME, forward::config::configure)?,
        paint: configure_app(paint::app::NAME, paint::config::configure)?,
        spotify: configure_app(spotify::app::NAME, spotify::config::configure)?,
        youtube: configure_app(youtube::app::NAME, youtube::config::configure)?,
        selection: configure_app(selection::app::NAME, selection::config::configure)?,
    });
}

fn configure_app<F, C>(name: &'static str, conf: F) -> Result<Option<C>, Box<dyn std::error::Error>> where
    F: FnOnce() -> Result<C, Box<dyn std::error::Error>>
{
    let items = ["yes", "no"];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("[apps] do you want to configure the {} application?", name))
        .default(0)
        .items(&items)
        .interact()?;

    return Ok(if items[selection] == "yes" {
        Some(conf()?)
    } else {
        None
    });
}

#[derive(Clone, Debug, PartialEq)]
pub enum In {
    Midi(MidiEvent),
    Server(ServerCommand),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Out {
    Midi(MidiEvent),
    Server(ServerCommand),
}

impl From<MidiEvent> for In {
    fn from(event: MidiEvent) -> Self {
        return In::Midi(event);
    }
}

impl From<MidiEvent> for Out {
    fn from(event: MidiEvent) -> Self {
        return Out::Midi(event);
    }
}

impl From<ServerCommand> for In {
    fn from(command: ServerCommand) -> Self {
        return In::Server(command);
    }
}

impl From<ServerCommand> for Out {
    fn from(command: ServerCommand) -> Self {
        return Out::Server(command);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn get_test_config() -> Config {
        return toml::from_str(r#"
            [forward]
            [youtube]
            api_key = "megaplop"
            playlist_id = "woohoo"
        "#).unwrap();
    }

    #[test]
    pub fn test_start_missing_app() {
        let app = get_test_config().start(
            "spotify",
            Arc::new(crate::midi::devices::default::DefaultFeatures::new()),
            Arc::new(crate::midi::devices::default::DefaultFeatures::new()),
        );

        assert!(app.is_none());
    }

    #[test]
    pub fn test_start_configured_app() {
        let app = get_test_config().start(
            "forward",
            Arc::new(crate::midi::devices::default::DefaultFeatures::new()),
            Arc::new(crate::midi::devices::default::DefaultFeatures::new()),
        );

        assert!(app.is_some());
        assert_eq!(app.unwrap().get_name(), "forward");
    }

    #[test]
    pub fn test_start_all_with_no_apps() {
        let config: Config = toml::from_str(r#"
        "#).unwrap();

        let apps = config.start_all(
            Arc::new(crate::midi::devices::default::DefaultFeatures::new()),
            Arc::new(crate::midi::devices::default::DefaultFeatures::new()),
        );

        assert_eq!(apps.len(), 0);
    }

    #[test]
    pub fn test_start_all_with_two_apps() {
        let apps = get_test_config().start_all(
            Arc::new(crate::midi::devices::default::DefaultFeatures::new()),
            Arc::new(crate::midi::devices::default::DefaultFeatures::new()),
        );

        assert_eq!(apps.iter().map(|app| app.get_name()).collect::<Vec<&str>>(), vec!["forward", "youtube"]);
    }
}
