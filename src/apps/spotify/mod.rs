use crate::server::Command;

pub mod app;
pub mod client;

pub mod authorization;

#[derive(Debug)]
pub enum Out<E> {
    Command(Command),
    Event(E),
}

#[derive(Debug, Clone)]
pub struct Config {
    pub authorization: authorization::Config,
    pub playlist_id: String,
}
