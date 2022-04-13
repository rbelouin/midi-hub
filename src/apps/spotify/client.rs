use std::future::Future;
use std::time::Instant;
use reqwest::{Client, Response, StatusCode};
use reqwest::header::HeaderMap;
use serde_json::Value;

#[derive(Clone, Copy, Debug)]
pub enum SpotifyError {
    NoDevices,
    ReqwestError,
    SerdeError,
    Unauthorized,
    Unknown,
}

pub mod albums {
    use serde::Deserialize;

    #[derive(Clone, Debug, Deserialize)]
    pub struct SpotifyAlbumImage {
        pub width: u16,
        pub height: u16,
        pub url: String,
    }

    #[derive(Clone, Debug, Deserialize)]
    pub struct SpotifyAlbum {
        pub images: Vec<SpotifyAlbumImage>,
    }
}

pub mod tracks {
    use serde::Deserialize;
    use super::albums::SpotifyAlbum;

    #[derive(Clone, Debug, Deserialize)]
    pub struct SpotifyTrack {
        pub id: String,
        pub name: String,
        pub uri: String,
        pub album: SpotifyAlbum,
    }
}

pub mod player {
    use serde::Deserialize;
    use serde_json::json;

    /// See https://developer.spotify.com/console/put-play/
    pub async fn play(token: String, track_uri: String) -> Result<(), super::SpotifyError> {
        let device = get_active_device(token.clone()).await?;

        super::log(format!("Playing track {} on device {}", track_uri, device.id), || async {
            return super::put(format!("https://api.spotify.com/v1/me/player/play?device_id={}", device.id), token, json!({
                "uris": vec![&track_uri]
            })).await;
        }).await?;

        return Ok(());
    }

    /// See https://developer.spotify.com/console/put-pause/
    pub async fn pause(token: String) -> Result<(), super::SpotifyError> {
        let device = get_active_device(token.clone()).await?;

        super::log(format!("Pausing track on device {}", device.id), || async {
            return super::put(format!("https://api.spotify.com/v1/me/player/pause?device_id={}", device.id), token, json!({})).await;
        }).await?;

        return Ok(());
    }

    #[derive(Clone, Debug, Deserialize)]
    pub struct SpotifyDeviceResponse {
        pub devices: Vec<SpotifyDevice>,
    }

    #[derive(Clone, Debug, Deserialize)]
    pub struct SpotifyDevice {
        pub id: String,
        pub is_active: bool,
        pub name: String,
    }

    /// We try to play/pause on the active device, but fall back to the first inactive one
    /// otherwise; as Spotify devices get rapidly treated as inactive.
    pub async fn get_active_device(token: String) -> Result<SpotifyDevice, super::SpotifyError> {
        let response = get_devices(token).await?;
        let first_device = response.devices.get(0).map(|d| d.clone());
        let active_device = response.devices.into_iter().find(|device| device.is_active);
        return active_device.or(first_device).ok_or(super::SpotifyError::NoDevices);
    }

    /// See https://developer.spotify.com/console/get-users-available-devices/
    pub async fn get_devices(token: String) -> Result<SpotifyDeviceResponse, super::SpotifyError> {
        return super::log("Get devices".to_string(), || async {
            return super::get(format!("https://api.spotify.com/v1/me/player/devices"), token).await?
                .json::<SpotifyDeviceResponse>()
                .await
                .map_err(|_| super::SpotifyError::SerdeError);
        }).await;
    }
}

pub mod playlists {
    use serde::Deserialize;
    use super::tracks::SpotifyTrack;

    #[derive(Clone, Debug, Deserialize)]
    pub struct SpotifyPlaylistResponse {
        pub href: String,
        pub items: Vec<SpotifyPlaylistItem>
    }

    #[derive(Clone, Debug, Deserialize)]
    pub  struct SpotifyPlaylistItem {
        pub track: SpotifyTrack,
    }

    pub async fn get_playlist_tracks(token: String, playlist_id: String) -> Result<Vec<SpotifyTrack>, super::SpotifyError> {
        return super::log(format!("Get tracks from playlist {}", playlist_id), || async {
            let response = super::get(format!("https://api.spotify.com/v1/playlists/{}/tracks", playlist_id), token).await?
                .json::<SpotifyPlaylistResponse>()
                .await
                .map_err(|_| super::SpotifyError::SerdeError)?;

            return Ok(response.items.iter().map(|item| item.track.clone()).collect());
        }).await;
    }
}

async fn log<F, Fut, T>(description: String, action: F) -> T where
    F: FnOnce() -> Fut,
    Fut: Future<Output = T>,
{
    let start = Instant::now();
    println!("[spotify] {}", description);
    let result = action().await;
    println!("[spotify] {} (done in {}ms)", description, (Instant::now() - start).as_millis());
    return result;
}

async fn get(url: String, token: String) -> Result<Response, SpotifyError> {
    let client = Client::new();
    let response = client.get(url)
        .headers(headers(token))
        .send()
        .await
        .map_err(|_| SpotifyError::ReqwestError)?;

    if response.status() == StatusCode::UNAUTHORIZED {
        return Err(SpotifyError::Unauthorized);
    } else {
        return Ok(response);
    }
}

async fn put(url: String, token: String, value: Value) -> Result<Response, SpotifyError> {
    let client = Client::new();
    let response = client.put(url)
        .headers(headers(token))
        .json(&value)
        .send()
        .await
        .map_err(|_| SpotifyError::ReqwestError)?;

    if response.status() == StatusCode::UNAUTHORIZED {
        return Err(SpotifyError::Unauthorized);
    } else {
        return Ok(response);
    }
}

fn headers(token: String) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("Authorization", format!("Bearer {}", token).parse().unwrap());
    return headers;
}

mod test {
    // 1. Visit https://developer.spotify.com/console/put-play/
    // 2. Get a token there, with user-modify-playback-state permissions
    // 3. Run `SPOTIFY_TOKEN=... cargo test --features spotify
    #[test]
    #[cfg(feature="spotify")]
    fn test_play_and_pause_track() {
        use std::time::Duration;
        use tokio::runtime::Builder;
        use tokio::time::sleep;

        let runtime = Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(runtime.spawn(async move {
            let token = std::env::var("SPOTIFY_TOKEN")
                .expect("SPOTIFY_TOKEN to be defined as an environment variable");

            let tracks = super::playlists::get_playlist_tracks(token.clone(), "1vsF6HQZWDv6BHPPBevJMG".to_string()).await
                .expect("Getting playlist tracks should work");

            let track_uri = tracks[0].uri.clone();
            super::player::play(token.clone(), track_uri).await
                .expect("Playing a track should work");

            sleep(Duration::from_millis(2000)).await;

            super::player::pause(token).await
                .expect("Pausing should work");
        })).unwrap();
    }
}
