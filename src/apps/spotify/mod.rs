pub mod app;
pub mod client;

pub mod authorization;

#[derive(Debug, Clone)]
pub struct Config {
    pub authorization: authorization::Config,
    pub playlist_id: String,
}
