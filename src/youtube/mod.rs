use crate::server::Command;

pub mod app;
pub mod client;

#[derive(Debug)]
pub enum Out<E> where E: std::fmt::Debug {
    Command(Command),
    Event(E),
}
