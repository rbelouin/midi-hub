use serde::{Serialize, Deserialize};

pub mod app;
pub mod server;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Command {
    Play(String),
}
