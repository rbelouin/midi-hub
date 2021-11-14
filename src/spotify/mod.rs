use serde::Deserialize;
use serde_json::json;

use tokio::runtime::Builder;
use tokio::sync::mpsc;

use std::{future::Future, sync::{Arc, Mutex}};

use reqwest::{StatusCode, header::HeaderMap};

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
    spawn: mpsc::Sender<SpotifyTask>,
}

impl SpotifyTaskSpawner {
    pub fn new(config: authorization::SpotifyAppConfig) -> SpotifyTaskSpawner {
        let config = Arc::new(config);
        let access_token = Arc::new(Mutex::new(None));
        let (send, mut recv) = mpsc::channel(32);

        let rt = Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let access_token_copy = Arc::clone(&access_token);
        let config_copy = Arc::clone(&config);
        std::thread::spawn(move || {
            rt.block_on(async move {
                while let Some(task) = recv.recv().await {
                    let config = Arc::clone(&config_copy);
                    let access_token = Arc::clone(&access_token_copy);
                    tokio::spawn(handle_spotify_task(config, access_token, task));
                }
            });
        });

        SpotifyTaskSpawner {
            config,
            access_token,
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
}

async fn handle_spotify_task(config: Arc<authorization::SpotifyAppConfig>, access_token: Arc<Mutex<Option<String>>>, task: SpotifyTask) {
    let SpotifyTask { action, playlist_id } = task;
    let _ = match action {
        SpotifyAction::Play { index } => with_access_token(config, access_token, |token| async {
            return start_or_resume_index(token, &playlist_id, index).await.map(|_track| ());
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
pub struct SpotifyTrack {
    id: String,
    name: String,
    uri: String,
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
