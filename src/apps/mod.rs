use std::convert::From;

use serde::{Serialize, Deserialize};
use tokio::sync::mpsc::error::{SendError, TryRecvError};

use crate::image::Image;
pub use crate::midi::Event as MidiEvent;
pub use crate::server::Command as ServerCommand;

pub mod forward;
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
    fn send(&mut self, event: MidiEvent) -> Result<(), SendError<MidiEvent>>;

    /// Poll events emitted by the application
    fn receive(&mut self) -> Result<Out, TryRecvError>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub forward: Option<forward::config::Config>,
    pub spotify: Option<spotify::config::Config>,
    pub youtube: Option<youtube::config::Config>,
    pub selection: Option<selection::config::Config>,
}

pub fn configure() -> Result<Config, Box<dyn std::error::Error>> {
    let mut configure_forward = String::new();
    let mut configure_spotify = String::new();
    let mut configure_youtube = String::new();
    let mut configure_selection = String::new();

    println!("[apps] do you want to configure the forward application? (yes|no)");
    std::io::stdin().read_line(&mut configure_forward)?;
    let configure_forward = configure_forward.trim();
    println!("");

    let forward = if configure_forward == "yes" {
        Some(forward::config::configure()?)
    } else {
        None
    };

    println!("[apps] do you want to configure the spotify application? (yes|no)");
    std::io::stdin().read_line(&mut configure_spotify)?;
    let configure_spotify = configure_spotify.trim();
    println!("");

    let spotify = if configure_spotify == "yes" {
        Some(spotify::config::configure()?)
    } else {
        None
    };

    println!("[apps] do you want to configure the youtube application? (yes|no)");
    std::io::stdin().read_line(&mut configure_youtube)?;
    let configure_youtube = configure_youtube.trim();
    println!("");

    let youtube = if configure_youtube == "yes" {
        Some(youtube::config::configure()?)
    } else {
        None
    };

    println!("[apps] do you want to configure the selection application? (yes|no)");
    std::io::stdin().read_line(&mut configure_selection)?;
    let configure_selection = configure_selection.trim();
    println!("");

    let selection = if configure_selection == "yes" {
        Some(selection::config::configure()?)
    } else {
        None
    };

    return Ok(Config {
        forward,
        spotify,
        youtube,
        selection,
    });
}

#[derive(Debug, PartialEq)]
pub enum Out {
    Midi(MidiEvent),
    Server(ServerCommand),
}

impl From<MidiEvent> for Out {
    fn from(event: MidiEvent) -> Self {
        return Out::Midi(event);
    }
}

impl From<ServerCommand> for Out {
    fn from(command: ServerCommand) -> Self {
        return Out::Server(command);
    }
}
