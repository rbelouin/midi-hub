use serde::{Serialize, Deserialize};

use dialoguer::{theme::ColorfulTheme, Input};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    pub api_key: String,
    pub playlist_id: String,
}

pub fn configure() -> Result<Config, Box<dyn std::error::Error>> {
    let api_key = Input::<String>::with_theme(&ColorfulTheme::default())
        .with_prompt("[youtube] please enter your api key:")
        .interact()?
        .trim()
        .to_string();

    let playlist_id = Input::<String>::with_theme(&ColorfulTheme::default())
        .with_prompt("[youtube] please enter the id of the playlist you want to play via midi-hub:")
        .interact()?
        .trim()
        .to_string();

    return Ok(Config {
        api_key,
        playlist_id,
    });
}
