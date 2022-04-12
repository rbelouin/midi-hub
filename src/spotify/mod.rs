use crate::server::Command;

pub mod app;
pub mod client;

pub mod authorization;
pub use authorization::SpotifyAuthorizationConfig;

#[derive(Debug)]
pub enum Out<E> {
    Command(Command),
    Event(E),
}

#[derive(Debug, Clone)]
pub struct SpotifyAppConfig {
    pub authorization: SpotifyAuthorizationConfig,
    pub playlist_id: String,
}
