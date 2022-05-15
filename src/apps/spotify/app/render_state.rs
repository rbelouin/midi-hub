use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::image::Image;
use super::app::*;
use super::app::PlaybackState::*;

const G: [u8; 3] = [0, 255, 0];
const W: [u8; 3] = [255, 255, 255];

pub async fn render_state_reactively(
    state: Arc<State>,
    terminate: Arc<AtomicBool>,
) {
    let rendered_index = Arc::new(Mutex::new(None));
    // render once in the beginning, since the state will be unchanged.
    render_state(Arc::clone(&state)).await;

    while terminate.load(Ordering::Relaxed) != true {
        let r_index = Arc::clone(&rendered_index).lock().unwrap().clone();
        let playback = Arc::clone(&state).playback.lock().unwrap().clone();

        match playback {
            PAUSED | PAUSING => {
                if r_index != None {
                    render_state(Arc::clone(&state)).await;
                    let mut rendered_index = rendered_index.lock().unwrap();
                    *rendered_index = None;
                }
            },
            REQUESTED(index) => {
                if r_index != Some(index) {
                    render_cover(Arc::clone(&state)).await;
                    render_state(Arc::clone(&state)).await;
                    let mut rendered_index = rendered_index.lock().unwrap();
                    *rendered_index = Some(index);
                }
            },
            PLAYING(index) => {
                if r_index != Some(index) {
                    render_state(Arc::clone(&state)).await;
                    let mut rendered_index = rendered_index.lock().unwrap();
                    *rendered_index = Some(index);
                }
            },
        }
        tokio::time::sleep(Duration::from_millis(60)).await;
    }
}

pub async fn render_state(state: Arc<State>) {
    render_logo(Arc::clone(&state)).await;
    render_highlighted_index(Arc::clone(&state)).await;
}

async fn render_logo(state: Arc<State>) {
    match state.output_transformer.from_image(get_logo()) {
        Err(err) => eprintln!("[spotify] could not render the spotify logo: {}", err),
        Ok(event) => {
            state.sender.send(event.into()).await.unwrap_or_else(|err| {
                eprintln!("[spotify] could send the logo event back to the router: {}", err)
            });
        },
    }
}

async fn render_highlighted_index(state: Arc<State>) {
    let playback = state.playback.lock().unwrap().clone();

    match playback {
        REQUESTED(index) | PLAYING(index) => match state.output_transformer.from_index_to_highlight(index) {
            Err(err) => eprintln!("[spotify] could not highlight the index {}: {}", index, err),
            Ok(event) => {
                state.sender.send(event.into()).await.unwrap_or_else(|err| {
                    eprintln!("[spotify] could not send the highlighting-index event back to the router: {}", err)
                });
            },
        },
        _ => {},
    }
}

async fn render_cover(state: Arc<State>) {
    let track = {
        let playback = state.playback.lock().unwrap().clone();
        match playback {
            PAUSED | PAUSING => None,
            PLAYING(index) | REQUESTED(index) => {
                let tracks = state.tracks.lock().unwrap();
                tracks.as_ref().map(|tracks| tracks[index as usize].clone())
            },
        }
    };

    match track {
        None => render_logo(state).await,
        Some(track) => {
            match track.album.images.last().map(|image| image.url.clone()) {
                None => {
                    eprintln!("[spotify] no cover found for track {}", track.uri);
                    render_logo(state).await
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
                        state.sender.send(event.into()).await.unwrap_or_else(|err| {
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
    use std::future::Future;
    use std::sync::Mutex;
    use std::time::Instant;

    use tokio::runtime::Builder;

    use crate::apps::spotify::config::Config;
    use crate::apps::spotify::client::{MockSpotifyApiClient, SpotifyTrack};
    use crate::midi::{Error, Event, EventTransformer};
    use crate::midi::features::{R, ImageRenderer};
    use super::*;


    #[test]
    fn render_state_when_working_transformer_and_no_playing_index_then_render_state() {
        const FAKE_EVENT_TRANSFORMER: FakeEventTransformer = FakeEventTransformer {};

        struct FakeEventTransformer {}
        impl ImageRenderer for FakeEventTransformer {
            fn from_image(&self, mut image: Image) -> R<Event> {
                let mut prefix = Vec::from("IMG".as_bytes());
                let mut bytes = vec![];
                bytes.append(&mut prefix);
                bytes.append(&mut image.bytes);
                return Ok(Event::SysEx(bytes));
            }
        }
        impl EventTransformer for FakeEventTransformer {
            fn into_index(&self, _event: Event) -> Result<Option<u16>, Error> {
                return Err(Error::Unsupported);
            }

            fn from_index_to_highlight(&self, index: u16) -> Result<Event, Error> {
                return Ok(Event::Midi([index as u8, index as u8, index as u8, index as u8]));
            }
        }

        let (sender, mut receiver) = tokio::sync::mpsc::channel::<Out>(32);

        let state = get_state_with(
            &FAKE_EVENT_TRANSFORMER,
            vec![],
            PAUSED,
            sender,
        );

        with_runtime(async move {
            render_state(state).await;
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
        impl ImageRenderer for FakeEventTransformer {
            fn from_image(&self, mut image: Image) -> R<Event> {
                let mut prefix = Vec::from("IMG".as_bytes());
                let mut bytes = vec![];
                bytes.append(&mut prefix);
                bytes.append(&mut image.bytes);
                return Ok(Event::SysEx(bytes));
            }
        }
        impl EventTransformer for FakeEventTransformer {
            fn into_index(&self, _event: Event) -> Result<Option<u16>, Error> {
                return Err(Error::Unsupported);
            }

            fn from_index_to_highlight(&self, index: u16) -> Result<Event, Error> {
                return Ok(Event::Midi([index as u8, index as u8, index as u8, index as u8]));
            }
        }

        let (sender, mut receiver) = tokio::sync::mpsc::channel::<Out>(32);

        let state = get_state_with(
            &FAKE_EVENT_TRANSFORMER,
            vec![],
            PLAYING(42),
            sender,
        );

        with_runtime(async move {
            render_state(state).await;
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

            fn from_index_to_highlight(&self, index: u16) -> Result<Event, Error> {
                return Ok(Event::Midi([index as u8, index as u8, index as u8, index as u8]));
            }
        }

        let (sender, mut receiver) = tokio::sync::mpsc::channel::<Out>(32);

        let state = get_state_with(
            &FAKE_EVENT_TRANSFORMER,
            vec![],
            PLAYING(42),
            sender,
        );

        with_runtime(async move {
            render_state(state).await;

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

            fn from_index_to_highlight(&self, _index: u16) -> Result<Event, Error> {
                return Err(Error::Unsupported);
            }
        }

        let (sender, mut receiver) = tokio::sync::mpsc::channel::<Out>(32);

        let state = get_state_with(
            &FAKE_EVENT_TRANSFORMER,
            vec![],
            PLAYING(42),
            sender,
        );

        with_runtime(async move {
            render_state(state).await;

            let event = receiver.recv().await;
            assert_eq!(event, None);
        });
    }

    fn get_state_with(
        transformer: &'static (dyn EventTransformer + Sync),
        tracks: Vec<SpotifyTrack>,
        playback: PlaybackState,
        sender: Sender<Out>,
    ) -> Arc<State> {
        let client = Box::new(MockSpotifyApiClient::new());

        let config = Config {
            playlist_id: "playlist_id".to_string(),
            client_id: "client_id".to_string(),
            client_secret: "client_secret".to_string(),
            refresh_token: "refresh_token".to_string(),
        };

        Arc::new(State {
            client,
            input_transformer: transformer,
            output_transformer: transformer,
            access_token: Mutex::new(Some("access_token".to_string())),
            last_action: Mutex::new(Instant::now()),
            tracks: Mutex::new(Some(tracks)),
            playback: Mutex::new(playback),
            config,
            sender,
        })
    }

    fn with_runtime<F>(f: F) -> F::Output where F: Future {
        Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(f)
    }
}
