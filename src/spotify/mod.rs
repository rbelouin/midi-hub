use serde::Deserialize;
use serde_json::json;

use tokio::runtime::Builder;
use tokio::sync::mpsc;
use tokio::time::sleep;

use std::{future::Future, sync::{Arc, Mutex}};
use std::time::{Duration, Instant};

use reqwest::{StatusCode, header::HeaderMap};

use crate::image::Image;
use crate::midi::{FromImage, FromImages, IntoIndex};

pub mod authorization;
pub use authorization::SpotifyAuthorizationConfig;

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
}

const DELAY: Duration = Duration::from_millis(5_000);

impl<E: 'static> SpotifyTaskSpawner<E> {
    pub fn new(config: SpotifyAppConfig, sender: mpsc::Sender<E>) -> SpotifyTaskSpawner<E> where
        E: FromImage<E>,
        E: FromImages<E>,
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
                render_playlist_tracks(Arc::clone(&state_copy), Arc::clone(&sender)).await;
                while let Some(event) = recv.recv().await {
                    let config = Arc::clone(&config_copy);
                    let state = Arc::clone(&state_copy);
                    let mut last_action = state.last_action.lock().unwrap();
                    if last_action.elapsed() > DELAY {
                        tokio::spawn(handle_spotify_task(Arc::clone(&config), Arc::clone(&state), Arc::clone(&sender), event.clone()));
                        tokio::spawn(reset_selected_covers(config, Arc::clone(&state), Arc::clone(&sender)));
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

async fn reset_selected_covers<E>(config: Arc<SpotifyAppConfig>, state: Arc<State>, sender: Arc<mpsc::Sender<E>>) where E: FromImages<E> {
    sleep(Duration::from_millis(5_000)).await;
    let playlist_id = &config.playlist_id.clone();
    let _ = with_access_token(config, state, |token| async {
        let tracks = playlist_tracks(token, playlist_id).await.unwrap_or(vec![]);
        let mut images = Vec::with_capacity(tracks.len());
        let fallback_image = Image { width: 64, height: 64, bytes: vec![0; 64*64*3] };
        for n in 0..tracks.len() {
            let image_url = tracks[n].album.images.last().map(|image| image.url.clone());
            if image_url.is_some() {
                images.push(Image::from_url(&image_url.unwrap()).await.unwrap_or(fallback_image.clone()));
            } else {
                images.push(fallback_image.clone());
            }
        };

        let event = E::from_images(images).map_err(|_| SpotifyResponseError::Unknown)?;
        return sender.send(event).await.map_err(|_| SpotifyResponseError::Unknown);
    }).await;
}

async fn handle_spotify_task<E>(config: Arc<SpotifyAppConfig>, state: Arc<State>, sender: Arc<mpsc::Sender<E>>, event_in: E) where
    E: FromImage<E>,
    E: IntoIndex
{
    let playlist_id = &config.playlist_id.clone();
    let _ = match event_in.into_index() {
        Ok(Some(index)) => with_access_token(config, state, |token| async {
            let track = start_or_resume_index(token, playlist_id, index.into()).await;
            let cover_url = track.clone().ok().map(|t| t.album.images.last().map(|i| i.url.clone())).flatten();
            match cover_url {
                Some(url) => {
                    let image = Image::from_url(&url).await.map_err(|_| ());
                    let event_out = image.and_then(|image| {
                        return E::from_image(image).map_err(|_| ());
                    });

                    match event_out {
                        Ok(event) => {
                            let _ = sender.send(event).await;
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
    let playlist_id = config.playlist_id.clone();
    let tracks = with_access_token(config, Arc::clone(&state), |token| async {
        return playlist_tracks(token, &playlist_id).await;
    }).await;

    match tracks {
        Err(_) => println!("[Spotify] could not pull tracks from playlist {}", playlist_id),
        Ok(tracks) => {
            let mut state_tracks = state.tracks.lock().unwrap();
            *state_tracks = Some(tracks);
        },
    }
}

async fn render_playlist_tracks<E>(state: Arc<State>, sender: Arc<mpsc::Sender<E>>) where E: FromImages<E> {
    let default_tracks = vec![];
    let guard = state.tracks.lock().unwrap();
    let tracks = guard.as_ref().unwrap_or(&default_tracks);

    let mut images = Vec::with_capacity(tracks.len());
    let fallback_image = Image { width: 64, height: 64, bytes: vec![0; 64*64*3] };

    for n in 0..tracks.len() {
        let image_url = tracks[n].album.images.last().map(|image| image.url.clone());
        if image_url.is_some() {
            images.push(Image::from_url(&image_url.unwrap()).await.unwrap_or(fallback_image.clone()));
        } else {
            images.push(fallback_image.clone());
        }
    };

    match E::from_images(images) {
        Err(_) => println!("[Spotify] could not render the playlist tracks"),
        Ok(event) => {
            let _ = sender.send(event).await;
        },
    }
}

#[derive(Debug, Copy, Clone)]
pub enum SpotifyResponseError {
    NotAuthorized,
    Unknown,
}

async fn with_access_token<A, F, Fut>(config: Arc<SpotifyAppConfig>, state: Arc<State>, f: F) -> Result<A, ()> where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Result<A, SpotifyResponseError>>,
{
    let token = state.access_token.lock().unwrap().clone();
    return match token {
        Some(token) => {
            println!("[Spotify] Found token in memory");
            match f(token.to_string()).await {
                Err(SpotifyResponseError::NotAuthorized) => {
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

#[derive(Deserialize, Debug, Clone)]
pub struct SpotifyDevice {
    id: String,
    is_active: bool,
    name: String,
}

#[derive(Deserialize, Debug)]
pub struct SpotifyDeviceResponse {
    devices: Vec<SpotifyDevice>,
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

    let device = get_active_device(&token).await?;

    println!("[Spotify] Playing track {} on device {}", track_uri, device.id);
    let response = client.put(format!("https://api.spotify.com/v1/me/player/play?device_id={}", device.id))
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

pub async fn _pause(token: String) -> Result<(), SpotifyResponseError> {
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

pub async fn get_devices(token: &String) -> Result<SpotifyDeviceResponse, SpotifyResponseError> {
    let client = reqwest::Client::new();

    println!("[Spotify] Get devices");
    let response = client.get(format!("https://api.spotify.com/v1/me/player/devices"))
        .headers(headers(token))
        .send()
        .await
        .map_err(|_err| SpotifyResponseError::Unknown)?;

    println!("Status: {}", response.status());
    if response.status() == StatusCode::UNAUTHORIZED {
        return Err(SpotifyResponseError::NotAuthorized);
    } else {
        let response = response
            .json::<SpotifyDeviceResponse>()
            .await
            .map_err(|_err| SpotifyResponseError::Unknown)?;

        return Ok(response);
    }
}

pub async fn get_active_device(token: &String) -> Result<SpotifyDevice, SpotifyResponseError> {
    let response = get_devices(token).await?;
    let first_device = response.devices.get(0).map(|d| d.clone());
    let active_device = response.devices.into_iter().find(|device| device.is_active);
    return active_device.or(first_device).ok_or(SpotifyResponseError::Unknown);
}

fn headers(token: &String) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("Authorization", format!("Bearer {}", token).parse().unwrap());
    return headers;
}
