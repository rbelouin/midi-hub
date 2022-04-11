use tokio::runtime::Builder;
use tokio::sync::mpsc;
use tokio::time::sleep;

use std::{future::Future, sync::{Arc, Mutex}};
use std::time::{Duration, Instant};

use crate::image::Image;
use crate::midi::{FromImage, FromImages, FromSelectedIndex, IntoIndex};
use crate::server::Command;

pub mod authorization;
pub use authorization::SpotifyAuthorizationConfig;

pub mod client;
use client::SpotifyError;
use client::tracks::SpotifyTrack;

#[derive(Debug)]
pub enum Out<E> where E: std::fmt::Debug {
    Command(Command),
    Event(E),
}

#[derive(Debug, Clone)]
pub struct SpotifyAppConfig {
    pub authorization: SpotifyAuthorizationConfig,
    pub playlist_id: String,
}

#[derive(Clone)]
pub struct SpotifyTaskSpawner<E> {
    config: Arc<SpotifyAppConfig>,
    state: Arc<State>,
    spawn: mpsc::Sender<E>,
}

struct State {
    access_token: Mutex<Option<String>>,
    last_action: Mutex<Instant>,
    tracks: Mutex<Option<Vec<SpotifyTrack>>>,
    playing: Mutex<Option<u16>>,
}

const DELAY: Duration = Duration::from_millis(5_000);

impl<E: 'static> SpotifyTaskSpawner<E> {
    pub fn new(config: SpotifyAppConfig, sender: mpsc::Sender<Out<E>>) -> SpotifyTaskSpawner<E> where
        E: FromImage<E>,
        E: FromImages<E>,
        E: FromSelectedIndex<E>,
        E: IntoIndex,
        E: Clone,
        E: std::fmt::Debug,
        E: std::marker::Send,
    {
        let config = Arc::new(config);
        let sender = Arc::new(sender);
        let state = Arc::new(State {
            access_token: Mutex::new(None),
            last_action: Mutex::new(Instant::now() - DELAY),
            tracks: Mutex::new(None),
            playing: Mutex::new(None),
        });

        let (send, mut recv) = mpsc::channel::<E>(32);

        let rt = Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let state_copy = Arc::clone(&state);
        let config_copy = Arc::clone(&config);
        std::thread::spawn(move || {
            rt.block_on(async move {
                pull_playlist_tracks(Arc::clone(&config_copy), Arc::clone(&state_copy)).await;
                render_spotify_logo(Arc::clone(&state_copy), Arc::clone(&sender)).await;
                while let Some(event) = recv.recv().await {
                    let config = Arc::clone(&config_copy);
                    let state = Arc::clone(&state_copy);
                    let mut last_action = state.last_action.lock().unwrap();
                    if last_action.elapsed() > DELAY {
                        tokio::spawn(handle_spotify_task(Arc::clone(&config), Arc::clone(&state), Arc::clone(&sender), event.clone()));
                        *last_action = Instant::now();
                    } else {
                        println!("Ignoring event: {:?}", event);
                    }
                }
            });
        });

        SpotifyTaskSpawner {
            config,
            state,
            spawn: send,
        }
    }

    pub fn handle(&self, event: E) where
        E: FromImage<E>,
        E: FromImages<E>,
        E: IntoIndex
    {
        match self.spawn.blocking_send(event) {
            Ok(()) => {},
            Err(_) => panic!("The shared runtime has shut down."),
        }
    }
}

pub fn login_sync(config: SpotifyAppConfig) -> Result<authorization::SpotifyTokenResponse, ()> {
    let runtime = Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();

    let config = Arc::new(config);
    return runtime.block_on(runtime.spawn(async move {
        let response = authorization::authorize(&config.authorization).await;
        return match response {
            Ok(token) => Ok(token),
            Err(_) => {
                println!("Error!");
                Err(())
            },
        }
    })).unwrap();
}

async fn handle_spotify_task<E>(config: Arc<SpotifyAppConfig>, state: Arc<State>, sender: Arc<mpsc::Sender<Out<E>>>, event_in: E) where
    E: FromImage<E>,
    E: FromImages<E>,
    E: FromSelectedIndex<E>,
    E: IntoIndex,
    E: std::fmt::Debug,
{
    let _ = match event_in.into_index() {
        Ok(Some(index)) => with_access_token(Arc::clone(&config), Arc::clone(&state), |token| async {
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

async fn pull_playlist_tracks(config: Arc<SpotifyAppConfig>, state: Arc<State>) {
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

async fn render_spotify_logo<E>(state: Arc<State>, sender: Arc<mpsc::Sender<Out<E>>>) where
    E: FromImage<E>,
    E: FromSelectedIndex<E>,
    E: std::fmt::Debug,
{
    match E::from_image(get_spotify_logo()) {
        Err(_) => println!("[Spotify] could not render the spotify logo"),
        Ok(event) => {
            let _ = sender.send(Out::Event(event)).await;
        },
    }

    let playing = state.playing.lock().unwrap().clone();
    match playing {
        Some(index) => match E::from_selected_index(index) {
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

async fn with_access_token<A, F, Fut>(config: Arc<SpotifyAppConfig>, state: Arc<State>, f: F) -> Result<A, ()> where
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

async fn fetch_and_store_access_token(config: Arc<SpotifyAppConfig>, state: Arc<State>) ->  Result<String, ()> {
    let token_response =  authorization::refresh_token(&config.authorization).await.unwrap();
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
        Some(track) => sender.send(Out::Command(Command::SpotifyPlay {
            track_id: format!("spotify:track:{}", track.id.clone()),
            access_token: token
        })).await
            .map(|_| track)
            .map_err(|_| SpotifyError::Unknown),
        _ => Err(SpotifyError::Unknown),
    }
}

pub const COLOR: [u8; 3] = [0, 255, 0];

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
