use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::image::Image;
use super::app::*;

const G: [u8; 3] = [0, 255, 0];
const W: [u8; 3] = [255, 255, 255];

pub async fn render_state_reactively(
    state: Arc<State>,
    sender: Arc<Sender<Out>>,
    terminate: Arc<AtomicBool>,
) {
    let rendered_index = Arc::new(Mutex::new(None));
    // render once in the beginning, since the state will be unchanged.
    render_state(Arc::clone(&state), Arc::clone(&sender)).await;

    while terminate.load(Ordering::Relaxed) != true {
        let r = Arc::clone(&rendered_index).lock().unwrap().clone();
        let p = Arc::clone(&state).playing.lock().unwrap().clone();

        // Render the cover of the track we’ve just started to play,
        // and only THEN render the logo + highlighted index.
        if r != p && p.is_some() {
            render_cover(Arc::clone(&state), Arc::clone(&sender)).await;
        }

        if r != p {
            render_state(Arc::clone(&state), Arc::clone(&sender)).await;
            {
                let mut rendered_index = rendered_index.lock().unwrap();
                *rendered_index = p;
            }
        }

        tokio::time::sleep(Duration::from_millis(60)).await;
    }
}

pub async fn render_state(state: Arc<State>, sender: Arc<Sender<Out>>) {
    render_logo(Arc::clone(&state), Arc::clone(&sender)).await;
    render_highlighted_index(Arc::clone(&state), Arc::clone(&sender)).await;
}

async fn render_logo(state: Arc<State>, sender: Arc<Sender<Out>>) {
    match state.output_transformer.from_image(get_logo()) {
        Err(err) => eprintln!("[spotify] could not render the spotify logo: {}", err),
        Ok(event) => {
            sender.send(event.into()).await.unwrap_or_else(|err| {
                eprintln!("[spotify] could send the logo event back to the router: {}", err)
            });
        },
    }
}

async fn render_highlighted_index(state: Arc<State>, sender: Arc<Sender<Out>>) {
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

async fn render_cover(state: Arc<State>, sender: Arc<Sender<Out>>) {
    let track = {
        let playing = state.playing.lock().unwrap();
        playing.as_ref().and_then(|index| {
            let tracks = state.tracks.lock().unwrap();
            tracks.as_ref().map(|tracks| tracks[*index as usize].clone())
        })
    };

    match track {
        None => render_logo(state, sender).await,
        Some(track) => {
            match track.album.images.last().map(|image| image.url.clone()) {
                None => {
                    eprintln!("[spotify] no cover found for track {}", track.uri);
                    render_logo(state, sender).await
                },
                Some(cover_url) => {
                    let image = Image::from_url(&cover_url).await.map_err(|err| {
                        eprintln!("[spotify] could not retrieve image: {:?}", err)
                    });

                    let event_out = image.and_then(|image| {
                        return state.output_transformer.from_image(image).map_err(|err| {
                            eprintln!("[spotify] could not transform image into a MIDI event: {}", err)
                        });
                    });

                    if let Ok(event) = event_out {
                        sender.send(event.into()).await.unwrap_or_else(|err| {
                            eprintln!("[spotify] could send the image back to the router: {}", err)
                        });

                        // Render the cover image for as long as throttling takes effect
                        tokio::time::sleep(super::app::DELAY).await;
                    }
                },
            }
        },
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
    fn render_state_when_working_transformer_and_no_playing_index_then_render_state() {
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
                render_state(state, sender).await;
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
    fn render_state_when_working_transformer_and_playing_index_then_render_logo_and_highlight_index() {
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
                render_state(state, sender).await;
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
    fn render_state_when_transformer_supports_only_highlighting_and_playing_index_then_and_highlight_index() {
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
                render_state(state, sender).await;

                let event = receiver.recv().await.unwrap();
                assert_eq!(event, Out::Midi(Event::Midi([42, 42, 42, 42])));

                let event = receiver.recv().await;
                assert_eq!(event, None);
            });
    }

    #[test]
    fn render_state_when_transformer_supports_nothing_and_playing_index_then_do_nothing() {
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
                render_state(state, sender).await;

                let event = receiver.recv().await;
                assert_eq!(event, None);
            });
    }
}
