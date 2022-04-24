use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {}

/// The application doesn’t need configuration at the moment
pub fn configure() -> Result<Config, Box<dyn std::error::Error>> {
    return Ok(Config {});
}
