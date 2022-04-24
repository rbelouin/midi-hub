use std::convert::From;
use tokio::sync::mpsc::error::{SendError, TryRecvError};

use crate::image::Image;
pub use crate::server::Command as ServerCommand;

pub mod forward;
pub mod spotify;
pub mod youtube;

pub trait App<I, O> {
    /// Exposing a name enables the router to log more meaningful information
    fn get_name(&self) -> &'static str;

    /// Color will be used by devices who can assign a color to "app selection" buttons
    fn get_color(&self) -> [u8; 3];

    /// Logo will be used by devices who can render a picture when the application is selected
    fn get_logo(&self) -> Image;

    /// Send an event to be handled by the application
    fn send(&self, event: I) -> Result<(), SendError<I>>;

    /// Poll events emitted by the application
    fn receive(&mut self) -> Result<O, TryRecvError>;
}

#[derive(Debug)]
pub enum Out<E> {
    Event(E),
    Server(ServerCommand),
}

impl<E> From<ServerCommand> for Out<E> {
    fn from(command: ServerCommand) -> Self {
        return Out::Server(command);
    }
}
