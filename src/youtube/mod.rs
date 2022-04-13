use crate::server::Command;

pub mod app;
pub mod client;

#[derive(Debug)]
pub enum Out<E> {
    Command(Command),
    Event(E),
}

#[derive(Clone, Debug)]
pub struct Config {
    pub api_key: String,
    pub playlist_id: String,
}
