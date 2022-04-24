use tokio::runtime::Builder;
use tokio::sync::mpsc;
use tokio::time::sleep;

use std::{future::Future, sync::{Arc, Mutex}};
use std::time::{Duration, Instant};

use crate::apps::{App, Out, ServerCommand};
use crate::image::Image;
use crate::midi::RichEvent;

use super::config::Config;
use super::client;
use super::client::SpotifyError;
use super::client::tracks::SpotifyTrack;

pub const NAME: &'static str = "spotify";
pub const COLOR: [u8; 3] = [0, 255, 0];

const DELAY: Duration = Duration::from_millis(5_000);

struct State {
    access_token: Mutex<Option<String>>,
    last_action: Mutex<Instant>,
    tracks: Mutex<Option<Vec<SpotifyTrack>>>,
    playing: Mutex<Option<u16>>,
}

pub struct Spotify<E> {
    in_sender: mpsc::Sender<E>,
    out_receiver: mpsc::Receiver<Out<E>>,
}

impl<E: 'static> Spotify<E> where E: RichEvent<E> {
    pub fn new(config: Config) -> Self {
        let config = Arc::new(config);
        let state = Arc::new(State {
            access_token: Mutex::new(None),
            last_action: Mutex::new(Instant::now() - DELAY),
            tracks: Mutex::new(None),
            playing: Mutex::new(None),
        });

        let (in_sender, mut in_receiver) = mpsc::channel::<E>(32);
        let (out_sender, out_receiver) = mpsc::channel::<Out<E>>(32);

        let rt = Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let config_copy = Arc::clone(&config);
        let out_sender = Arc::new(out_sender);
        std::thread::spawn(move || {
            rt.block_on(async move {
                pull_playlist_tracks(Arc::clone(&config_copy), Arc::clone(&state)).await;
                render_spotify_logo(Arc::clone(&state), Arc::clone(&out_sender)).await;
                while let Some(event) = in_receiver.recv().await {
                    let config = Arc::clone(&config_copy);
                    let state = Arc::clone(&state);
                    let time_elapsed = {
                        let last_action = state.last_action.lock().unwrap();
                        last_action.elapsed()
                    };

                    if time_elapsed > DELAY {
                        tokio::spawn(handle_spotify_task(Arc::clone(&config), Arc::clone(&state), Arc::clone(&out_sender), event.clone()));
                    } else {
                        println!("Ignoring event: {:?}", event);
                    }
                }
            });
        });

        Spotify {
            in_sender,
            out_receiver,
        }
    }
}

impl<E: 'static> App<E, Out<E>> for Spotify<E> where E: RichEvent<E> {
    fn get_name(&self) -> &'static str {
        return NAME;
    }

    fn get_color(&self) -> [u8; 3] {
        return COLOR;
    }

    fn get_logo(&self) -> Image {
        let g = [0, 255, 0];
        let w = [255, 255, 255];
        return Image {
            width: 8,
            height: 8,
            bytes: vec![
                g, g, g, g, g, g, g, g,
                g, g, w, w, w, w, g, g,
                g, w, g, g, g, g, w, g,
                g, g, w, w, w, w, g, g,
                g, w, g, g, g, g, w, g,
                g, g, w, w, w, w, g, g,
                g, w, g, g, g, g, w, g,
                g, g, g, g, g, g, g, g,
            ].concat(),
        };
    }

    fn send(&self, event: E) -> Result<(), mpsc::error::SendError<E>> {
        return self.in_sender.blocking_send(event);
    }

    fn receive(&mut self) -> Result<Out<E>, mpsc::error::TryRecvError> {
        return self.out_receiver.try_recv();
    }
}

async fn handle_spotify_task<E>(config: Arc<Config>, state: Arc<State>, sender: Arc<mpsc::Sender<Out<E>>>, event_in: E) where E: RichEvent<E> {
    let _ = match event_in.into_index() {
        Ok(Some(index)) => with_access_token(Arc::clone(&config), Arc::clone(&state), |token| async {
            {
                let mut last_action = state.last_action.lock().unwrap();
                *last_action = Instant::now();
            }

            let s = Arc::clone(&state);
            let playing = *s.playing.lock().unwrap();
            if playing == Some(index) {
                let res = client::player::pause(token).await;
                if res.is_ok() {
                    {
                        let s = Arc::clone(&state);
                        let mut playing = s.playing.lock().unwrap();
                        *playing = None;
                    }
                    render_spotify_logo(Arc::clone(&state), Arc::clone(&sender)).await;
                }
                return res;
            }

            let track = start_or_resume_index(token, Arc::clone(&state), Arc::clone(&sender), index.into()).await;
            if track.is_ok() {
                let s = Arc::clone(&state);
                let mut playing = s.playing.lock().unwrap();
                *playing = Some(index);
            }

            let cover_url = track.clone().ok().map(|t| t.album.images.last().map(|i| i.url.clone())).flatten();
            match cover_url {
                Some(url) => {
                    let image = Image::from_url(&url).await.map_err(|_| ());
                    let event_out = image.and_then(|image| {
                        return E::from_image(image).map_err(|_| ());
                    });

                    match event_out {
                        Ok(event) => {
                            let _ = sender.send(Out::Event(event)).await;
                            sleep(DELAY).await;
                            pull_playlist_tracks(Arc::clone(&config), Arc::clone(&state)).await;
                            render_spotify_logo(Arc::clone(&state), Arc::clone(&sender)).await;
                        },
                        Err(_) => {
                            println!("Could not download and decode {}", url);
                        },
                    }
                },
                None => println!("No cover found for track {:?}", track.as_ref().map(|t| t.id.clone()).map_err(|_err| ())),
            }
            return track.map(|_t| ());
        }).await,
        _ => {
            return ();
        },
    };
}

async fn pull_playlist_tracks(config: Arc<Config>, state: Arc<State>) {
    let tracks = with_access_token(Arc::clone(&config), Arc::clone(&state), |token| async {
        return client::playlists::get_playlist_tracks(token, Arc::clone(&config).playlist_id.clone()).await;
    }).await;

    match tracks {
        Err(_) => println!("[Spotify] could not pull tracks from playlist {}", config.playlist_id),
        Ok(tracks) => {
            let mut state_tracks = state.tracks.lock().unwrap();
            *state_tracks = Some(tracks);
        },
    }
}

async fn render_spotify_logo<E>(state: Arc<State>, sender: Arc<mpsc::Sender<Out<E>>>) where E: RichEvent<E> {
    match E::from_image(get_spotify_logo()) {
        Err(_) => println!("[Spotify] could not render the spotify logo"),
        Ok(event) => {
            let _ = sender.send(Out::Event(event)).await;
        },
    }

    let playing = state.playing.lock().unwrap().clone();
    match playing {
        Some(index) => match E::from_index_to_highlight(index) {
            Ok(event) => {
                let _ = sender.send(Out::Event(event)).await;
            },
            Err(e) => {
                println!("Could not select index: {} ({})", index, e);
            },
        },
        None => {},
    };
}

async fn with_access_token<A, F, Fut>(config: Arc<Config>, state: Arc<State>, f: F) -> Result<A, ()> where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Result<A, SpotifyError>>,
{
    let token = state.access_token.lock().unwrap().clone();
    return match token {
        Some(token) => {
            println!("[Spotify] Found token in memory");
            match f(token.to_string()).await {
                Err(SpotifyError::Unauthorized) => {
                    println!("[Spotify] Retrying because of expired token");
                    let token = fetch_and_store_access_token(config, state).await?;
                    return f(token).await.map_err(|_err| ());
                },
                Err(_) => Err(()),
                Ok(a) => Ok(a),
            }
        },
        None => {
            println!("[Spotify] No token in memory");
            let token = fetch_and_store_access_token(config, state).await?;
            return f(token).await.map_err(|_err| ());
        },
    };
}

async fn fetch_and_store_access_token(config: Arc<Config>, state: Arc<State>) ->  Result<String, ()> {
    let token_response =  client::authorization::refresh_token(
        &config.client_id,
        &config.client_secret,
        &config.refresh_token
    ).await.unwrap();
    let mut new_token = state.access_token.lock().unwrap();
    *new_token = Some(token_response.access_token.clone());
    return Ok(token_response.access_token.clone());
}

async fn start_or_resume_index<E>(token: String, state: Arc<State>, sender: Arc<mpsc::Sender<Out<E>>>, index: usize) -> Result<SpotifyTrack, SpotifyError> where
    E: std::fmt::Debug,
{
    println!("[Spotify] Playing track {}", index);
    let track = state.tracks.lock().unwrap().as_ref()
        .and_then(|tracks| tracks.get(index))
        .map(|track| track.clone());

    return match track {
        Some(track) => sender.send(ServerCommand::SpotifyPlay {
            track_id: format!("spotify:track:{}", track.id.clone()),
            access_token: token
        }.into()).await
            .map(|_| track)
            .map_err(|_| SpotifyError::Unknown),
        _ => Err(SpotifyError::Unknown),
    }
}

pub fn get_spotify_logo() -> Image {
    let g = [0, 255, 0];
    let w = [255, 255, 255];

    return Image {
        width: 8,
        height: 8,
        bytes: vec![
            g, g, g, g, g, g, g, g,
            g, g, w, w, w, w, g, g,
            g, w, g, g, g, g, w, g,
            g, g, w, w, w, w, g, g,
            g, w, g, g, g, g, w, g,
            g, g, w, w, w, w, g, g,
            g, w, g, g, g, g, w, g,
            g, g, g, g, g, g, g, g,
        ].concat(),
    };
}
