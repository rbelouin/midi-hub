use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    pub apps: Box<crate::apps::Config>,
}

pub fn configure() -> Result<Config, Box<dyn std::error::Error>> {
    println!("[selection] configure the following apps for your selection:");
    let mut apps = crate::apps::configure()?;

    if apps.selection.is_some() {
        println!("[selection] what kind of sorcery are you trying to do?? the selection app cannot be configured recursively!");
        apps.selection = None;
    }

    return Ok(Config {
        apps: Box::new(apps),
    });
}
