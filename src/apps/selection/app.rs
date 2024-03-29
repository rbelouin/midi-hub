use std::sync::Arc;

use tokio::sync::mpsc::{Sender, Receiver, channel};
use tokio::sync::mpsc::error::{SendError, TryRecvError};

use crate::apps::{App, In, Out};

use crate::midi::Image;
use crate::midi::features::Features;

use super::config::Config;

pub const NAME: &str = "selection";
pub const COLOR: [u8; 3] = [255, 255, 255];

pub struct Selection {
    pub apps: Vec<Box<dyn App>>,
    pub selected_app: usize,
    input_features: Arc<dyn Features + Sync + Send>,
    output_features: Arc<dyn Features + Sync + Send>,
    out_sender: Sender<Out>,
    out_receiver: Receiver<Out>,
}

impl Selection {
    pub fn new(
        config: Config,
        input_features: Arc<dyn Features + Sync + Send>,
        output_features: Arc<dyn Features + Sync + Send>,
    ) -> Self {
        let (out_sender, out_receiver) = channel::<Out>(32);
        let selection = Selection {
            apps: config.apps.start_all(Arc::clone(&input_features), Arc::clone(&output_features)),
            selected_app: 0,
            input_features,
            output_features,
            out_sender,
            out_receiver,
        };

        selection.render_app_colors();

        return selection;
    }

    fn render_app_colors(&self) {
        self.output_features.from_app_colors(self.apps.iter().map(|app| app.get_color()).collect())
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
    fn send(&mut self, event: In) -> Result<(), SendError<In>> {
        match event {
            In::Midi(event) => {
                let selected_app = self.input_features.into_app_index(event.clone()).ok().flatten()
                    .and_then(|app_index| {
                        let selected_app = self.apps.get_mut(app_index as usize);
                        if selected_app.is_some() {
                            self.selected_app = app_index as usize;
                        }
                        return selected_app;
                    });

                selected_app
                    .map(|selected_app| {
                        println!("[selection] selecting {}", selected_app.get_name());
                        self.output_features.from_color_palette(vec![[0, 0, 0]; 8])
                            .map_err(|err| format!("[selection] could not transform color palette: {}", err))
                            .and_then(|event| self.out_sender.blocking_send(event.into())
                                .map_err(|err| format!("[selection] could not clean the color palette: {}", err)))
                            .unwrap_or_else(|err| eprintln!("{}", err));

                        self.output_features.from_image(selected_app.get_logo())
                            .map_err(|err| format!("[selection] could not transform the image: {}", err))
                            .and_then(|event| self.out_sender.blocking_send(event.into())
                                .map_err(|err| format!("[selection] could not send the image: {}", err)))
                            .unwrap_or_else(|err| eprintln!("{}", err));

                        selected_app.on_select();
                    })
                    .unwrap_or_else(|| {
                        match self.apps.get_mut(self.selected_app) {
                            Some(app) => app.send(event.into())
                                .unwrap_or_else(|err| eprintln!("[selection][{}] could not send event: {}", app.get_name(), err)),
                            None => eprintln!("No app found for index: {}", self.selected_app),
                        }
                    });
                Ok(())
            },
            In::Server(command)  => {
                for app in &mut self.apps {
                    app.send(command.clone().into()).unwrap_or_else(|err| {
                        println!("[selection] could not forward server command to {}: {}", app.get_name(), err);
                    });
                }
                Ok(())
            },
        }
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

    fn on_select(&mut self) {}
}

#[cfg(test)]
mod test {
    use crate::midi::Event;
    use crate::midi::features::{R, AppSelector, Features};
    use crate::apps;
    use super::*;

    struct TestFeatures {}
    impl AppSelector for TestFeatures {
        fn from_app_colors(&self, app_colors: Vec<[u8; 3]>) -> R<Event> {
            let mut bytes = vec![];
            for app_color in &app_colors {
                bytes.push(app_color[0]);
                bytes.push(app_color[1]);
                bytes.push(app_color[2]);
            }
            return Ok(Event::SysEx(bytes));
        }
    }
    impl Features for TestFeatures {}

    #[test]
    fn test_render_app_colors_on_instantiation() {
        let mut selection_app = Selection::new(
            Config {
                apps: Box::new(apps::Config {
                    forward: None,
                    paint: None,
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
            Arc::new(TestFeatures {}),
            Arc::new(TestFeatures {}),
        );

        let event = selection_app.receive().expect("an event should be received");

        assert_eq!(event, Event::SysEx(vec![0, 255, 0, 255, 0, 0]).into());
    }
}
