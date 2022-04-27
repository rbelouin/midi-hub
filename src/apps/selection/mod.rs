use tokio::sync::mpsc::{Sender, Receiver, channel};
use tokio::sync::mpsc::error::{SendError, TryRecvError};

use crate::apps;
use crate::apps::{App, Out};

use crate::midi::{Event, EventTransformer, Writer};

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

        let (out_sender, out_receiver) = channel::<Out>(32);
        let selection = Selection {
            apps: vec![Box::new(spotify_app), Box::new(youtube_app)],
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

    // This one will be hard to test until we let Selection accept more generic apps
    pub fn send<W: Writer>(&mut self, writer: &mut W, event: Event) -> Result<(), SendError<Event>> {
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
                let _ = self.output_transformer.from_image(selected_app.get_logo())
                    .and_then(|event| writer.write(event));
            },
            _ => {
                match self.apps.get(self.selected_app) {
                    Some(app) => app.send(event)
                        .unwrap_or_else(|err| eprintln!("[selection][{}] could not send event: {}", app.get_name(), err)),
                    None => eprintln!("No app found for index: {}", self.selected_app),
                }
            },
        }
        Ok(())
    }

    // This one will be hard to test until we let Selection accept more generic apps
    pub fn receive(&mut self) -> Result<Out, TryRecvError> {
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
            apps::spotify::config::Config {
                playlist_id: "playlist_id".to_string(),
                client_id: "client_id".to_string(),
                client_secret: "client_secret".to_string(),
                refresh_token: "refresh_token".to_string(),
            },
            apps::youtube::config::Config {
                api_key: "api_key".to_string(),
                playlist_id: "playlist_id".to_string(),
            },
            &TRANSFORMER,
            &TRANSFORMER,
        );

        let event = selection_app.receive().expect("an event should be received");

        assert_eq!(event, Event::SysEx(vec![0, 255, 0, 255, 0, 0]).into());
    }
}
