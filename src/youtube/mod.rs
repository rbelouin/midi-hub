use serde::{Serialize, Deserialize};

pub mod app;
pub mod client;
pub mod server;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Command {
    Play(String),
}

#[derive(Debug)]
pub enum Out<E> where E: std::fmt::Debug {
    Command(Command),
    Event(E),
}
