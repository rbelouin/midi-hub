use tokio::runtime::Builder;
use tokio::sync::mpsc;
use tokio::time::sleep;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::apps::App;
use crate::image::Image;
use crate::midi::EventTransformer;

use super::super::config::Config;
use super::super::client::*;

use super::access_token::*;
use super::playback::*;
use super::render_logo::*;

pub const NAME: &'static str = "spotify";
pub const COLOR: [u8; 3] = [0, 255, 0];

const DELAY: Duration = Duration::from_millis(5_000);

pub type In = crate::apps::In;
pub type Out = crate::apps::Out;
pub type Sender<T> = tokio::sync::mpsc::Sender<T>;
pub type Receiver<T> = tokio::sync::mpsc::Receiver<T>;

pub struct State {
    pub client: Box<dyn SpotifyApiClient + Send + Sync>,
    pub input_transformer: &'static (dyn EventTransformer + Sync),
    pub output_transformer: &'static (dyn EventTransformer + Sync),
    pub access_token: Mutex<Option<String>>,
    pub last_action: Mutex<Instant>,
    pub tracks: Mutex<Option<Vec<SpotifyTrack>>>,
    pub playing: Mutex<Option<u16>>,
}

pub struct Spotify {
    in_sender: Sender<In>,
    out_receiver: Receiver<Out>,
}

impl Spotify {
    pub fn new(
        config: Config,
        client: Box<dyn SpotifyApiClient + Send + Sync>,
        input_transformer: &'static (dyn EventTransformer + Sync),
        output_transformer: &'static (dyn EventTransformer + Sync),
    ) -> Self {
        let config = Arc::new(config);
        let state = Arc::new(State {
            client,
            input_transformer,
            output_transformer,
            access_token: Mutex::new(None),
            last_action: Mutex::new(Instant::now() - DELAY),
            tracks: Mutex::new(None),
            playing: Mutex::new(None),
        });

        let (in_sender, in_receiver) = mpsc::channel::<In>(32);
        let (out_sender, out_receiver) = mpsc::channel::<Out>(32);
        let out_sender = Arc::new(out_sender);

        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        std::thread::spawn(move || {
            runtime.block_on(async move {
                let poll_state_config = Arc::clone(&config);
                let poll_state_state = Arc::clone(&state);
                let poll_state_sender = Arc::clone(&out_sender);
                tokio::spawn(async move {
                    poll_state(poll_state_config, poll_state_state, poll_state_sender).await;
                });

                let listen_config = Arc::clone(&config);
                let listen_state = Arc::clone(&state);
                let listen_sender = Arc::clone(&out_sender);
                listen_events(listen_config, listen_state, listen_sender, in_receiver).await;
            });
        });

        let spotify = Spotify {
            in_sender,
            out_receiver,
        };

        return spotify;
    }
}

impl App for Spotify {
    fn get_name(&self) -> &'static str {
        return NAME;
    }

    fn get_color(&self) -> [u8; 3] {
        return COLOR;
    }

    fn get_logo(&self) -> Image {
        return get_logo();
    }

    fn send(&mut self, event: In) -> Result<(), mpsc::error::SendError<In>> {
        return self.in_sender.blocking_send(event);
    }

    fn receive(&mut self) -> Result<Out, mpsc::error::TryRecvError> {
        return self.out_receiver.try_recv();
    }
}

async fn listen_events(
    config: Arc<Config>,
    state: Arc<State>,
    out_sender: Arc<Sender<Out>>,
    mut in_receiver: Receiver<In>,
) {
    pull_playlist_tracks(Arc::clone(&config), Arc::clone(&state)).await;
    render_logo(Arc::clone(&state), Arc::clone(&out_sender)).await;
    while let Some(event) = in_receiver.recv().await {
        let config = Arc::clone(&config);
        let state = Arc::clone(&state);
        let time_elapsed = {
            let last_action = state.last_action.lock().unwrap();
            last_action.elapsed()
        };

        if time_elapsed > DELAY {
            tokio::spawn(handle_spotify_task(Arc::clone(&config), Arc::clone(&state), Arc::clone(&out_sender), event));
        } else {
            println!("Ignoring event: {:?}", event);
        }
    }
}

async fn poll_state(config: Arc<Config>, state: Arc<State>, out_sender: Arc<Sender<Out>>) {
    loop {
        with_access_token(Arc::clone(&config), Arc::clone(&state), |token| async {
            let playback_state = state.client.get_playback_state(token).await.map_err(|err| {
                eprintln!("error: {:?}", err);
                err
            })?;
            let playing_index = if let Some(playback_state) = playback_state {
                if playback_state.is_playing {
                    state.tracks.lock()
                        .expect("should be able to lock state.tracks")
                        .as_ref()
                        .and_then(|tracks| {
                            for i in 0..tracks.len() {
                                if tracks[i].id == playback_state.item.id {
                                    return Some(i as u16);
                                }
                            }
                            return None;
                        })
                } else {
                    None
                }
            } else {
                None
            };

            let has_changed = {
                let mut playing = state.playing.lock()
                    .expect("should be able to lock state.playing");
                let previous_value = playing.clone();
                *playing = playing_index;
                playing_index != previous_value
            };

            if has_changed {
                render_logo(Arc::clone(&state), Arc::clone(&out_sender)).await;
            }

            Ok(())
        }).await.unwrap_or_else(|_| {
            eprintln!("[spotify] error when polling and updating state")
        });
        std::thread::sleep(Duration::from_millis(1_000));
    }
}

async fn handle_spotify_task(config: Arc<Config>, state: Arc<State>, sender: Arc<Sender<Out>>, event: In) {
    match event {
        In::Midi(event) => {
            match state.input_transformer.into_index(event) {
                Ok(Some(index)) => {
                    {
                        let mut last_action = state.last_action.lock().unwrap();
                        *last_action = Instant::now();
                    }

                    if let Some(track) = play_or_pause(Arc::clone(&state), Arc::clone(&sender), index).await {
                        let cover_url = track.album.images.last().map(|image| {
                            image.url.clone()
                        });

                        match cover_url {
                            Some(url) => {
                                let image = Image::from_url(&url).await.map_err(|err| {
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

                                    sleep(DELAY).await;
                                    pull_playlist_tracks(Arc::clone(&config), Arc::clone(&state)).await;
                                }
                            },
                            None => {
                                eprintln!("[spotify] no cover found for track {}", track.uri)
                            },
                        }
                    }

                    render_logo(Arc::clone(&state), Arc::clone(&sender)).await;
                },
                _ => {},
            }
        },
        _ => {},
    }
}

async fn pull_playlist_tracks(config: Arc<Config>, state: Arc<State>) {
    let tracks = with_access_token(Arc::clone(&config), Arc::clone(&state), |token| async {
        return state.client.get_playlist_tracks(token, Arc::clone(&config).playlist_id.clone()).await;
    }).await;

    match tracks {
        Err(_) => println!("[Spotify] could not pull tracks from playlist {}", config.playlist_id),
        Ok(tracks) => {
            let mut state_tracks = state.tracks.lock().unwrap();
            *state_tracks = Some(tracks);
        },
    }
}
