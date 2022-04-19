use serde::Deserialize;

pub mod app;
pub mod client;

pub mod authorization;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub authorization: authorization::Config,
    pub playlist_id: String,
}
