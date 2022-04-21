use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    pub api_key: String,
    pub playlist_id: String,
}

pub fn configure() -> Result<Config, Box<dyn std::error::Error>> {
    let mut api_key = String::new();
    let mut playlist_id = String::new();

    println!("[youtube] please enter your api key: ");
    std::io::stdin().read_line(&mut api_key)?;
    let api_key = api_key.trim().to_string();
    println!("");

    println!("[youtube] please enter the id of the playlist you want to play via midi-hub:");
    std::io::stdin().read_line(&mut playlist_id)?;
    let playlist_id = playlist_id.trim().to_string();
    println!("");

    return Ok(Config {
        api_key,
        playlist_id,
    });
}
