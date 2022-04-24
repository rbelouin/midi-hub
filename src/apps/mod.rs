use std::convert::From;
use tokio::sync::mpsc::error::{SendError, TryRecvError};

use crate::image::Image;
pub use crate::midi::Event as MidiEvent;
pub use crate::server::Command as ServerCommand;

pub mod forward;
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
    fn send(&self, event: MidiEvent) -> Result<(), SendError<MidiEvent>>;

    /// Poll events emitted by the application
    fn receive(&mut self) -> Result<Out, TryRecvError>;
}

#[derive(Debug)]
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
