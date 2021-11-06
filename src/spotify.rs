use serde::Deserialize;
use serde_json::json;

use tokio::runtime::Builder;
use tokio::sync::mpsc;

use reqwest::header::HeaderMap;

use std::io::{Error, ErrorKind};

#[derive(Debug, Clone)]
pub enum SpotifyAction {
    Play { index: usize },
    #[allow(dead_code)]
    Pause,
}

#[derive(Debug, Clone)]
pub struct SpotifyTask {
    pub action: SpotifyAction,
    pub token: String,
    pub playlist_id: String,
}

#[derive(Clone)]
pub struct SpotifyTaskSpawner {
    spawn: mpsc::Sender<SpotifyTask>,
}

impl SpotifyTaskSpawner {
    pub fn new() -> SpotifyTaskSpawner {
        let (send, mut recv) = mpsc::channel(32);
        let rt = Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        std::thread::spawn(move || {
            rt.block_on(async move {
                while let Some(task) = recv.recv().await {
                    tokio::spawn(handle_spotify_task(task));
                }
            });
        });

        SpotifyTaskSpawner {
            spawn: send,
        }
    }

    pub fn spawn_task(&self, task: SpotifyTask) {
        match self.spawn.blocking_send(task) {
            Ok(()) => {},
            Err(_) => panic!("The shared runtime has shut down."),
        }
    }
}

async fn handle_spotify_task(task: SpotifyTask) {
    let SpotifyTask { action, token, playlist_id } = task;
    let result = match action {
        SpotifyAction::Play { index } => start_or_resume_index(&token, &playlist_id, index).await.map(|_track| ()),
        SpotifyAction::Pause => pause(&token).await,
    };

    match result {
        Ok(_) => {},
        Err(err) => println!("Error: {}", err),
    }
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

pub async fn start_or_resume_index(token: &String, playlist_id: &String, index: usize) -> Result<SpotifyTrack, Box<dyn std::error::Error>> {
    println!("[Spotify] Playing track {} from playlist {}", index, playlist_id);
    let tracks = playlist_tracks(token, playlist_id).await?;
    return match tracks.get(index) {
        Some(track) => start_or_resume_track(token, &track.uri).await.map(|()| track.clone()),
        None      => Err(Box::new(Error::from(ErrorKind::NotFound))),
    }
}

pub async fn playlist_tracks(token: &String, playlist_id: &String) -> Result<Vec<SpotifyTrack>, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();

    println!("[Spotify] Get tracks from playlist {}", playlist_id);
    let response = client.get(format!("https://api.spotify.com/v1/playlists/{}/tracks", playlist_id))
        .headers(headers(token))
        .send()
        .await?
        .json::<SpotifyPlaylistResponse>()
        .await?;

    return Ok(response.items.iter().map(|item| item.track.clone()).collect());
}

pub async fn start_or_resume_track(token: &String, track_uri: &String) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();

    println!("[Spotify] Playing track {}", track_uri);
    client.put(format!("https://api.spotify.com/v1/me/player/play"))
        .headers(headers(token))
        .json(&json!({
            "uris": vec![&track_uri]
        }))
        .send()
        .await?;

    return Ok(());
}

pub async fn pause(token: &String) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();

    println!("[Spotify] Pausing the track");
    client.put(format!("https://api.spotify.com/v1/me/player/pause"))
        .headers(headers(token))
        .send()
        .await?;

    return Ok(());
}

fn headers(token: &String) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("Authorization", format!("Bearer {}", token).parse().unwrap());
    return headers;
}
