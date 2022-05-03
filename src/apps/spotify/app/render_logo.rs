use std::sync::Arc;

use crate::image::Image;
use super::app::*;

const G: [u8; 3] = [0, 255, 0];
const W: [u8; 3] = [255, 255, 255];

pub async fn render_logo(state: Arc<State>, sender: Arc<Sender<Out>>) {
    match state.output_transformer.from_image(get_logo()) {
        Err(err) => eprintln!("[spotify] could not render the spotify logo: {}", err),
        Ok(event) => {
            sender.send(event.into()).await.unwrap_or_else(|err| {
                eprintln!("[spotify] could send the logo event back to the router: {}", err)
            });
        },
    }

    let playing = state.playing.lock().unwrap().clone();
    match playing {
        Some(index) => match state.output_transformer.from_index_to_highlight(index) {
            Err(err) => eprintln!("[spotify] could not highlight the index {}: {}", index, err),
            Ok(event) => {
                sender.send(event.into()).await.unwrap_or_else(|err| {
                    eprintln!("[spotify] could not send the highlighting-index event back to the router: {}", err)
                });
            },
        },
        None => {},
    }
}

pub fn get_logo() -> Image {
    return Image {
        width: 8,
        height: 8,
        bytes: vec![
            G, G, G, G, G, G, G, G,
            G, G, W, W, W, W, G, G,
            G, W, G, G, G, G, W, G,
            G, G, W, W, W, W, G, G,
            G, W, G, G, G, G, W, G,
            G, G, W, W, W, W, G, G,
            G, W, G, G, G, G, W, G,
            G, G, G, G, G, G, G, G,
        ].concat(),
    };
}

#[cfg(test)]
mod test {
    use std::sync::Mutex;
    use std::time::Instant;

    use tokio::runtime::Builder;

    use crate::apps::spotify::client::MockSpotifyApiClient;
    use crate::midi::{Error, Event, EventTransformer};
    use super::*;


    #[test]
    fn render_logo_when_working_transformer_and_no_playing_index_then_render_logo() {
        const FAKE_EVENT_TRANSFORMER: FakeEventTransformer = FakeEventTransformer {};

        struct FakeEventTransformer {}
        impl EventTransformer for FakeEventTransformer {
            fn into_index(&self, _event: Event) -> Result<Option<u16>, Error> {
                return Err(Error::Unsupported);
            }

            fn into_app_index(&self, _event: Event) -> Result<Option<u16>, Error> {
                return Err(Error::Unsupported);
            }

            fn from_image(&self, mut image: Image) -> Result<Event, Error> {
                let mut prefix = Vec::from("IMG".as_bytes());
                let mut bytes = vec![];
                bytes.append(&mut prefix);
                bytes.append(&mut image.bytes);
                return Ok(Event::SysEx(bytes));
            }

            fn from_index_to_highlight(&self, index: u16) -> Result<Event, Error> {
                return Ok(Event::Midi([index as u8, index as u8, index as u8, index as u8]));
            }

            fn from_app_colors(&self, _app_colors: Vec<[u8; 3]>) -> Result<Event, Error> {
                return Err(Error::Unsupported);
            }
        }

        let (sender, mut receiver) = tokio::sync::mpsc::channel::<Out>(32);

        let client = MockSpotifyApiClient::new();
        let state = Arc::new(State {
            client: Box::new(client),
            input_transformer: &FAKE_EVENT_TRANSFORMER,
            output_transformer: &FAKE_EVENT_TRANSFORMER,
            access_token: Mutex::new(None),
            last_action: Mutex::new(Instant::now()),
            tracks: Mutex::new(None),
            playing: Mutex::new(None),
        });

        let sender = Arc::new(sender);

        Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(async move {
                render_logo(state, sender).await;
                let event = receiver.recv().await.unwrap();

                assert_eq!(event, Out::Midi(Event::SysEx(vec![
                    [b'I', b'M', b'G'],
                    G, G, G, G, G, G, G, G,
                    G, G, W, W, W, W, G, G,
                    G, W, G, G, G, G, W, G,
                    G, G, W, W, W, W, G, G,
                    G, W, G, G, G, G, W, G,
                    G, G, W, W, W, W, G, G,
                    G, W, G, G, G, G, W, G,
                    G, G, G, G, G, G, G, G,
                ].concat())));

                let event = receiver.recv().await;
                assert_eq!(event, None);
            });
    }

    #[test]
    fn render_logo_when_working_transformer_and_playing_index_then_render_logo_and_highlight_index() {
        const FAKE_EVENT_TRANSFORMER: FakeEventTransformer = FakeEventTransformer {};

        struct FakeEventTransformer {}
        impl EventTransformer for FakeEventTransformer {
            fn into_index(&self, _event: Event) -> Result<Option<u16>, Error> {
                return Err(Error::Unsupported);
            }

            fn into_app_index(&self, _event: Event) -> Result<Option<u16>, Error> {
                return Err(Error::Unsupported);
            }

            fn from_image(&self, mut image: Image) -> Result<Event, Error> {
                let mut prefix = Vec::from("IMG".as_bytes());
                let mut bytes = vec![];
                bytes.append(&mut prefix);
                bytes.append(&mut image.bytes);
                return Ok(Event::SysEx(bytes));
            }

            fn from_index_to_highlight(&self, index: u16) -> Result<Event, Error> {
                return Ok(Event::Midi([index as u8, index as u8, index as u8, index as u8]));
            }

            fn from_app_colors(&self, _app_colors: Vec<[u8; 3]>) -> Result<Event, Error> {
                return Err(Error::Unsupported);
            }
        }

        let (sender, mut receiver) = tokio::sync::mpsc::channel::<Out>(32);

        let client = MockSpotifyApiClient::new();
        let state = Arc::new(State {
            client: Box::new(client),
            input_transformer: &FAKE_EVENT_TRANSFORMER,
            output_transformer: &FAKE_EVENT_TRANSFORMER,
            access_token: Mutex::new(None),
            last_action: Mutex::new(Instant::now()),
            tracks: Mutex::new(None),
            playing: Mutex::new(Some(42)),
        });

        let sender = Arc::new(sender);

        Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(async move {
                render_logo(state, sender).await;
                let event = receiver.recv().await.unwrap();

                assert_eq!(event, Out::Midi(Event::SysEx(vec![
                    [b'I', b'M', b'G'],
                    G, G, G, G, G, G, G, G,
                    G, G, W, W, W, W, G, G,
                    G, W, G, G, G, G, W, G,
                    G, G, W, W, W, W, G, G,
                    G, W, G, G, G, G, W, G,
                    G, G, W, W, W, W, G, G,
                    G, W, G, G, G, G, W, G,
                    G, G, G, G, G, G, G, G,
                ].concat())));

                let event = receiver.recv().await.unwrap();
                assert_eq!(event, Out::Midi(Event::Midi([42, 42, 42, 42])));

                let event = receiver.recv().await;
                assert_eq!(event, None);
            });
    }

    #[test]
    fn render_logo_when_transformer_supports_only_highlighting_and_playing_index_then_and_highlight_index() {
        const FAKE_EVENT_TRANSFORMER: FakeEventTransformer = FakeEventTransformer {};

        struct FakeEventTransformer {}
        impl EventTransformer for FakeEventTransformer {
            fn into_index(&self, _event: Event) -> Result<Option<u16>, Error> {
                return Err(Error::Unsupported);
            }

            fn into_app_index(&self, _event: Event) -> Result<Option<u16>, Error> {
                return Err(Error::Unsupported);
            }

            fn from_image(&self, _image: Image) -> Result<Event, Error> {
                return Err(Error::Unsupported);
            }

            fn from_index_to_highlight(&self, index: u16) -> Result<Event, Error> {
                return Ok(Event::Midi([index as u8, index as u8, index as u8, index as u8]));
            }

            fn from_app_colors(&self, _app_colors: Vec<[u8; 3]>) -> Result<Event, Error> {
                return Err(Error::Unsupported);
            }
        }

        let (sender, mut receiver) = tokio::sync::mpsc::channel::<Out>(32);

        let client = MockSpotifyApiClient::new();
        let state = Arc::new(State {
            client: Box::new(client),
            input_transformer: &FAKE_EVENT_TRANSFORMER,
            output_transformer: &FAKE_EVENT_TRANSFORMER,
            access_token: Mutex::new(None),
            last_action: Mutex::new(Instant::now()),
            tracks: Mutex::new(None),
            playing: Mutex::new(Some(42)),
        });

        let sender = Arc::new(sender);

        Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(async move {
                render_logo(state, sender).await;

                let event = receiver.recv().await.unwrap();
                assert_eq!(event, Out::Midi(Event::Midi([42, 42, 42, 42])));

                let event = receiver.recv().await;
                assert_eq!(event, None);
            });
    }

    #[test]
    fn render_logo_when_transformer_supports_nothing_and_playing_index_then_do_nothing() {
        const FAKE_EVENT_TRANSFORMER: FakeEventTransformer = FakeEventTransformer {};

        struct FakeEventTransformer {}
        impl EventTransformer for FakeEventTransformer {
            fn into_index(&self, _event: Event) -> Result<Option<u16>, Error> {
                return Err(Error::Unsupported);
            }

            fn into_app_index(&self, _event: Event) -> Result<Option<u16>, Error> {
                return Err(Error::Unsupported);
            }

            fn from_image(&self, _image: Image) -> Result<Event, Error> {
                return Err(Error::Unsupported);
            }

            fn from_index_to_highlight(&self, _index: u16) -> Result<Event, Error> {
                return Err(Error::Unsupported);
            }

            fn from_app_colors(&self, _app_colors: Vec<[u8; 3]>) -> Result<Event, Error> {
                return Err(Error::Unsupported);
            }
        }

        let (sender, mut receiver) = tokio::sync::mpsc::channel::<Out>(32);

        let client = MockSpotifyApiClient::new();
        let state = Arc::new(State {
            client: Box::new(client),
            input_transformer: &FAKE_EVENT_TRANSFORMER,
            output_transformer: &FAKE_EVENT_TRANSFORMER,
            access_token: Mutex::new(None),
            last_action: Mutex::new(Instant::now()),
            tracks: Mutex::new(None),
            playing: Mutex::new(Some(42)),
        });

        let sender = Arc::new(sender);

        Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(async move {
                render_logo(state, sender).await;

                let event = receiver.recv().await;
                assert_eq!(event, None);
            });
    }
}
