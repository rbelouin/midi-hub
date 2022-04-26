use crate::apps;
use crate::apps::App;

use crate::midi::EventTransformer;

pub struct Selection {
    pub apps: Vec<Box<dyn App>>,
    pub selected_app: usize,
}

impl Selection {
    pub fn new(
        spotify: apps::spotify::config::Config,
        youtube: apps::youtube::config::Config,
        input_transformer: &'static (dyn EventTransformer + Sync),
        output_transformer: &'static (dyn EventTransformer + Sync),
    ) -> Self {
        let spotify_app = apps::spotify::app::Spotify::new(
            spotify.clone(),
            input_transformer,
            output_transformer,
        );

        let youtube_app = apps::youtube::app::Youtube::new(
            youtube.clone(),
            input_transformer,
            output_transformer,
        );

        return Selection {
            apps: vec![Box::new(spotify_app), Box::new(youtube_app)],
            selected_app: 0,
        };
    }
}
