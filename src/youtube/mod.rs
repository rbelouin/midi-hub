use crate::server::Command;

pub mod app;
pub mod client;

#[derive(Debug)]
pub enum Out<E> {
    Command(Command),
    Event(E),
}
