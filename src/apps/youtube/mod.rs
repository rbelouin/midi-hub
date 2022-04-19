use serde::Deserialize;

pub mod app;
pub mod client;

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub api_key: String,
    pub playlist_id: String,
}
