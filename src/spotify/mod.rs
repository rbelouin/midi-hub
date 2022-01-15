use serde::Deserialize;
use serde_json::json;

use tokio::runtime::Builder;
use tokio::sync::mpsc;

use std::{future::Future, sync::{Arc, Mutex}};
use std::time::{Duration, Instant};

use reqwest::{StatusCode, header::HeaderMap};

use crate::image::{Pixel, compress_from_url, compress_8x8};

pub mod authorization;

#[derive(Debug, Clone)]
pub enum SpotifyAction {
    Play { index: usize },
    #[allow(dead_code)]
    Pause,
}

#[derive(Debug, Clone)]
pub struct SpotifyTask {
    pub action: SpotifyAction,
    pub playlist_id: String,
}

#[derive(Clone)]
pub struct SpotifyTaskSpawner {
    config: Arc<authorization::SpotifyAppConfig>,
    access_token: Arc<Mutex<Option<String>>>,
    cover_pixels: Arc<Mutex<Option<Vec<Pixel>>>>,
    spawn: mpsc::Sender<SpotifyTask>,
}

const DELAY: Duration = Duration::from_millis(5_000);

impl SpotifyTaskSpawner {
    pub fn new(config: authorization::SpotifyAppConfig) -> SpotifyTaskSpawner {
        let config = Arc::new(config);
        let access_token = Arc::new(Mutex::new(None));
        let cover_pixels = Arc::new(Mutex::new(None));
        let (send, mut recv) = mpsc::channel(32);
        let last_action = Arc::new(Mutex::new(Instant::now() - DELAY));

        let rt = Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let access_token_copy = Arc::clone(&access_token);
        let cover_pixels_copy = Arc::clone(&cover_pixels);
        let config_copy = Arc::clone(&config);
        let last_action_copy = Arc::clone(&last_action);
        std::thread::spawn(move || {
            rt.block_on(async move {
                while let Some(task) = recv.recv().await {
                    let config = Arc::clone(&config_copy);
                    let access_token = Arc::clone(&access_token_copy);
                    let cover_pixels = Arc::clone(&cover_pixels_copy);
                    let mut last_action = last_action_copy.lock().unwrap();
                    if last_action.elapsed() > DELAY {
                        tokio::spawn(handle_spotify_task(config, access_token, cover_pixels, task));
                        *last_action = Instant::now();
                    } else {
                        println!("Ignoring task: {:?}", task);
                    }
                }
            });
        });

        SpotifyTaskSpawner {
            config,
            access_token,
            cover_pixels,
            spawn: send,
        }
    }

    pub fn login_sync(&self) -> Result<authorization::SpotifyTokenResponse, ()> {
        let runtime = Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();

        let config = Arc::clone(&self.config);
        return runtime.block_on(runtime.spawn(async move {
            let response = authorization::authorize(config.as_ref()).await;
            return match response {
                Ok(token) => Ok(token),
                Err(_) => {
                    println!("Error!");
                    Err(())
                },
            }
        })).unwrap();
    }

    pub fn spawn_task(&self, task: SpotifyTask) {
        match self.spawn.blocking_send(task) {
            Ok(()) => {},
            Err(_) => panic!("The shared runtime has shut down."),
        }
    }

    pub fn cover_pixels(&self) -> Option<Vec<Pixel>> {
        let value = self.cover_pixels.lock().unwrap().clone();
        if value != None {
            let mut cover_pixel = self.cover_pixels.lock().unwrap();
            *cover_pixel = None;
        }
        return value;
    }
}

async fn handle_spotify_task(config: Arc<authorization::SpotifyAppConfig>, access_token: Arc<Mutex<Option<String>>>,  cover_pixels: Arc<Mutex<Option<Vec<Pixel>>>>, task: SpotifyTask) {
    let SpotifyTask { action, playlist_id } = task;
    let _ = match action {
        SpotifyAction::Play { index } => with_access_token(config, access_token, |token| async {
            let track = start_or_resume_index(token, &playlist_id, index).await;
            let cover_url = track.clone().ok().map(|t| t.album.images.last().map(|i| i.url.clone())).flatten();
            match cover_url {
                Some(url) => {
                    let pixels = compress_from_url(url.clone(), compress_8x8).await;
                    let mut new_cover_pixels = cover_pixels.lock().unwrap();
                    match pixels {
                        Ok(pixels) => {
                            *new_cover_pixels = Some(pixels);
                        },
                        Err(_) => {
                            println!("Could not compress {}", url);
                            *new_cover_pixels = None;
                        },
                    }
                },
                None => println!("No cover found for track {:?}", track.as_ref().map(|t| t.id.clone()).map_err(|_err| ())),
            }
            return track.map(|_t| ());
        }).await,
        SpotifyAction::Pause => with_access_token(config, access_token, |token| async {
            return pause(token).await;
        }).await,
    };
}

#[derive(Debug, Copy, Clone)]
pub enum SpotifyResponseError {
    NotAuthorized,
    Unknown,
}

async fn with_access_token<A, F, Fut>(config: Arc<authorization::SpotifyAppConfig>, access_token: Arc<Mutex<Option<String>>>, f: F) -> Result<A, ()> where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Result<A, SpotifyResponseError>>,
{
    let token = access_token.lock().unwrap().clone();
    return match token {
        Some(token) => {
            println!("[Spotify] Found token in memory");
            match f(token.to_string()).await {
                Err(SpotifyResponseError::NotAuthorized) => {
                    println!("[Spotify] Retrying because of expired token");
                    let token = fetch_and_store_access_token(config, access_token).await?;
                    return f(token).await.map_err(|_err| ());
                },
                Err(_) => Err(()),
                Ok(a) => Ok(a),
            }
        },
        None => {
            println!("[Spotify] No token in memory");
            let token = fetch_and_store_access_token(config, access_token).await?;
            return f(token).await.map_err(|_err| ());
        },
    };
}

async fn fetch_and_store_access_token(config: Arc<authorization::SpotifyAppConfig>, access_token: Arc<Mutex<Option<String>>>) ->  Result<String, ()> {
    let token_response =  authorization::refresh_token(config.as_ref()).await.unwrap();
    let mut new_token = access_token.lock().unwrap();
    *new_token = Some(token_response.access_token.clone());
    return Ok(token_response.access_token.clone());
}

#[derive(Deserialize, Debug, Clone)]
pub struct SpotifyAlbumImage {
    width: u16,
    height: u16,
    url: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SpotifyAlbum {
    images: Vec<SpotifyAlbumImage>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SpotifyTrack {
    id: String,
    name: String,
    uri: String,
    album: SpotifyAlbum,
}

#[derive(Deserialize, Debug)]
pub  struct SpotifyPlaylistItem {
    track: SpotifyTrack,
}

#[derive(Deserialize, Debug)]
pub struct SpotifyPlaylistResponse {
    href: String,
    items: Vec<SpotifyPlaylistItem>
}

pub async fn start_or_resume_index(token: String, playlist_id: &String, index: usize) -> Result<SpotifyTrack, SpotifyResponseError> {
    println!("[Spotify] Playing track {} from playlist {}", index, playlist_id);
    let tracks = playlist_tracks(token.clone(), playlist_id).await?;
    return match tracks.get(index) {
        Some(track) => start_or_resume_track(token, &track.uri).await.map(|()| track.clone()),
        None      => Err(SpotifyResponseError::Unknown),
    }
}

pub async fn playlist_tracks(token: String, playlist_id: &String) -> Result<Vec<SpotifyTrack>, SpotifyResponseError> {
    let client = reqwest::Client::new();

    println!("[Spotify] Get tracks from playlist {}", playlist_id);
    let response = client.get(format!("https://api.spotify.com/v1/playlists/{}/tracks", playlist_id))
        .headers(headers(&token))
        .send()
        .await
        .map_err(|_err| SpotifyResponseError::Unknown)?;

    if response.status() == StatusCode::UNAUTHORIZED {
        return Err(SpotifyResponseError::NotAuthorized);
    } else {
        let response = response
            .json::<SpotifyPlaylistResponse>()
            .await
            .map_err(|_err| SpotifyResponseError::Unknown)?;

        return Ok(response.items.iter().map(|item| item.track.clone()).collect());
    }
}

pub async fn start_or_resume_track(token: String, track_uri: &String) -> Result<(), SpotifyResponseError> {
    let client = reqwest::Client::new();

    println!("[Spotify] Playing track {}", track_uri);
    let response = client.put(format!("https://api.spotify.com/v1/me/player/play"))
        .headers(headers(&token))
        .json(&json!({
            "uris": vec![&track_uri]
        }))
        .send()
        .await
        .map_err(|_err| SpotifyResponseError::Unknown)?;

    if response.status() == StatusCode::UNAUTHORIZED {
        return Err(SpotifyResponseError::NotAuthorized);
    } else {
        return Ok(());
    }
}

pub async fn pause(token: String) -> Result<(), SpotifyResponseError> {
    let client = reqwest::Client::new();

    println!("[Spotify] Pausing the track");
    let response = client.put(format!("https://api.spotify.com/v1/me/player/pause"))
        .headers(headers(&token))
        .send()
        .await
        .map_err(|_err| SpotifyResponseError::Unknown)?;

    if response.status() == StatusCode::UNAUTHORIZED {
        return Err(SpotifyResponseError::NotAuthorized);
    } else {
        return Ok(());
    }
}

fn headers(token: &String) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("Authorization", format!("Bearer {}", token).parse().unwrap());
    return headers;
}
