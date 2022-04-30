use std::future::Future;
use std::time::Instant;
use reqwest::{Client, Response, StatusCode};
use reqwest::header::HeaderMap;

#[derive(Clone, Copy, Debug)]
pub enum SpotifyError {
    ReqwestError,
    SerdeError,
    Unauthorized,
    Unknown,
}

pub mod authorization {
    use base64::encode;
    use reqwest::header::HeaderMap;
    use serde::Deserialize;

    #[derive(Clone, Debug, Deserialize)]
    pub struct SpotifyTokenResponse {
        pub access_token: String,
        pub token_type: String,
        pub scope: Option<String>,
        pub expires_in: i16,
        pub refresh_token: Option<String>,
    }

    pub async fn request_token(
        client_id: &String,
        client_secret: &String,
        code: &String,
    ) -> Result<SpotifyTokenResponse, Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();
        let response = client.post("https://accounts.spotify.com/api/token")
            .headers(prepare_headers(client_id, client_secret))
            .body(querystring::stringify(vec![
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", "http://localhost:12345/callback"),
            ]))
            .send()
            .await?;

        return Ok(response
            .json::<SpotifyTokenResponse>()
            .await?);
    }

    pub async fn refresh_token(
        client_id: &String,
        client_secret: &String,
        refresh_token: &String,
    ) -> Result<SpotifyTokenResponse, Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();
        let response = client.post("https://accounts.spotify.com/api/token")
            .headers(prepare_headers(client_id, client_secret))
            .body(querystring::stringify(vec![
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
            ]))
            .send()
            .await?;

        return Ok(response
            .json::<SpotifyTokenResponse>()
            .await?);
    }

    fn prepare_headers(client_id: &String, client_secret: &String) -> HeaderMap {
        let base64_authorization = encode(format!("{}:{}", client_id, client_secret));
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", format!("Basic {}", base64_authorization).parse().unwrap());
        headers.insert("Content-Type", "application/x-www-form-urlencoded".parse().unwrap());
        return headers;
    }
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

fn headers(token: String) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("Authorization", format!("Bearer {}", token).parse().unwrap());
    return headers;
}
