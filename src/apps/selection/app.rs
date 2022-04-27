use tokio::sync::mpsc::{Sender, Receiver, channel};
use tokio::sync::mpsc::error::{SendError, TryRecvError};

use crate::apps;
use crate::apps::{App, Out};

use crate::midi::{Event, EventTransformer, Image};

use super::config::Config;

pub const NAME: &str = "selection";
pub const COLOR: [u8; 3] = [255, 255, 255];

pub struct Selection {
    pub apps: Vec<Box<dyn App>>,
    pub selected_app: usize,
    input_transformer: &'static (dyn EventTransformer + Sync),
    output_transformer: &'static (dyn EventTransformer + Sync),
    out_sender: Sender<Out>,
    out_receiver: Receiver<Out>,
}

impl Selection {
    pub fn new(
        config: Config,
        input_transformer: &'static (dyn EventTransformer + Sync),
        output_transformer: &'static (dyn EventTransformer + Sync),
    ) -> Self {
        let mut apps: Vec<Box<dyn App>> = vec![];

        if let Some(forward_config) = config.apps.forward {
            apps.push(Box::new(apps::forward::app::Forward::new(
                forward_config,
                input_transformer,
                output_transformer,
            )));
        }

        if let Some(spotify_config) = config.apps.spotify {
            apps.push(Box::new(apps::spotify::app::Spotify::new(
                spotify_config,
                input_transformer,
                output_transformer,
            )));
        }

        if let Some(youtube_config) = config.apps.youtube {
            apps.push(Box::new(apps::youtube::app::Youtube::new(
                youtube_config,
                input_transformer,
                output_transformer,
            )));
        }

        let (out_sender, out_receiver) = channel::<Out>(32);
        let selection = Selection {
            apps,
            selected_app: 0,
            input_transformer,
            output_transformer,
            out_sender,
            out_receiver,
        };

        selection.render_app_colors();

        return selection;
    }

    fn render_app_colors(&self) {
        self.output_transformer.from_app_colors(self.apps.iter().map(|app| app.get_color()).collect())
            .map_err(|err| format!("[selection] could not render app colors: {}", err))
            .and_then(|event| self.out_sender.blocking_send(event.into())
                .map_err(|err| format!("[selection] could not send app colors: {}", err)))
            .unwrap_or_else(|err| eprintln!("{}", err));
    }
}

impl App for Selection {
    fn get_name(&self) -> &'static str {
        return NAME;
    }

    fn get_color(&self) -> [u8; 3] {
        return COLOR;
    }

    fn get_logo(&self) -> Image {
        return Image { width: 0, height: 0, bytes: vec![] };
    }

    // This one will be hard to test until we let Selection accept more generic apps
    fn send(&mut self, event: Event) -> Result<(), SendError<Event>> {
        let selected_app = self.input_transformer.into_app_index(event.clone()).ok().flatten()
            .and_then(|app_index| {
                let selected_app = self.apps.get(app_index as usize);
                if selected_app.is_some() {
                    self.selected_app = app_index as usize;
                }
                return selected_app;
            });

        match selected_app {
            Some(selected_app) => {
                println!("[selection] selecting {}", selected_app.get_name());
                self.output_transformer.from_image(selected_app.get_logo())
                    .map_err(|err| format!("[selection] could not transform the image: {}", err))
                    .and_then(|event| self.out_sender.blocking_send(event.into())
                        .map_err(|err| format!("[selection] could not send the image: {}", err)))
                    .unwrap_or_else(|err| eprintln!("{}", err));
            },
            _ => {
                match self.apps.get_mut(self.selected_app) {
                    Some(app) => app.send(event)
                        .unwrap_or_else(|err| eprintln!("[selection][{}] could not send event: {}", app.get_name(), err)),
                    None => eprintln!("No app found for index: {}", self.selected_app),
                }
            },
        }
        Ok(())
    }

    // This one will be hard to test until we let Selection accept more generic apps
    fn receive(&mut self) -> Result<Out, TryRecvError> {
        if let Ok(out) = self.out_receiver.try_recv() {
            return Ok(out);
        }

        if self.apps.len() > self.selected_app {
            return self.apps[self.selected_app].receive();
        } else {
            return Err(TryRecvError::Disconnected);
        }
    }
}

#[cfg(test)]
mod test {
    use crate::midi::{Error, Event, Image};
    use super::*;

    struct Transformer {}
    impl EventTransformer for Transformer {
        fn into_index(&self, _event: Event) -> Result<Option<u16>, Error> { Err(Error::Unsupported) }
        fn into_app_index(&self, _event: Event) -> Result<Option<u16>, Error> { Err(Error::Unsupported) }
        fn from_image(&self, _image: Image) -> Result<Event, Error> { Err(Error::Unsupported) }
        fn from_index_to_highlight(&self, _index: u16) -> Result<Event, Error> { Err(Error::Unsupported) }
        fn from_app_colors(&self, app_colors: Vec<[u8; 3]>) -> Result<Event, Error> {
            let mut bytes = vec![];
            for app_color in &app_colors {
                bytes.push(app_color[0]);
                bytes.push(app_color[1]);
                bytes.push(app_color[2]);
            }
            return Ok(Event::SysEx(bytes));
        }
    }

    const TRANSFORMER: Transformer = Transformer {};

    #[test]
    fn test_render_app_colors_on_instantiation() {
        let mut selection_app = Selection::new(
            Config {
                apps: Box::new(apps::Config {
                    forward: None,
                    spotify: Some(apps::spotify::config::Config {
                        playlist_id: "playlist_id".to_string(),
                        client_id: "client_id".to_string(),
                        client_secret: "client_secret".to_string(),
                        refresh_token: "refresh_token".to_string(),
                    }),
                    youtube: Some(apps::youtube::config::Config {
                        api_key: "api_key".to_string(),
                        playlist_id: "playlist_id".to_string(),
                    }),
                    selection: None,
                }),
            },
            &TRANSFORMER,
            &TRANSFORMER,
        );

        let event = selection_app.receive().expect("an event should be received");

        assert_eq!(event, Event::SysEx(vec![0, 255, 0, 255, 0, 0]).into());
    }
}
