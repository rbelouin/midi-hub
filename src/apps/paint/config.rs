use serde::{Serialize, Deserialize};

/// Add (de)serializable attributes to this structure
/// to make the Paint application configurable.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {}

/// This function is supposed to onboard the user with configuration,
/// prompting them questions to create an instance of Config at the end.
pub fn configure() -> Result<Config, Box<dyn std::error::Error>> {
    return Ok(Config {});
}
