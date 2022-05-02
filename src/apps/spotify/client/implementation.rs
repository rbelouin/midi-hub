use std::future::Future;
use std::time::Instant;

use base64::encode;
use reqwest::{Client, Response, StatusCode};
use reqwest::header::HeaderMap;

use super::*;

impl From<reqwest::Error> for SpotifyApiError {
    fn from(err: reqwest::Error) -> SpotifyApiError {
        return SpotifyApiError::Other(Box::new(err));
    }
}

pub const SPOTIFY_API_CLIENT: SpotifyApiClient = SpotifyApiClient {};

pub struct SpotifyApiClient {}

#[async_trait]
impl SpotifyApiClientInterface for SpotifyApiClient {
    async fn request_token(
        &self,
        client_id: &String,
        client_secret: &String,
        code: &String,
    ) -> SpotifyApiResult<SpotifyTokenResponse> {
        let client = reqwest::Client::new();
        let response = client.post("https://accounts.spotify.com/api/token")
            .headers(prepare_headers(client_id, client_secret))
            .body(querystring::stringify(vec![
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", "http://localhost:12345/callback"),
            ]))
            .send()
            .await
            .map_err(SpotifyApiError::from)?;

        return Ok(response
            .json::<SpotifyTokenResponse>()
            .await
            .map_err(SpotifyApiError::from)?);
    }

    async fn refresh_token(
        &self,
        client_id: &String,
        client_secret: &String,
        refresh_token: &String,
    ) -> SpotifyApiResult<SpotifyTokenResponse> {
        let client = reqwest::Client::new();
        let response = client.post("https://accounts.spotify.com/api/token")
            .headers(prepare_headers(client_id, client_secret))
            .body(querystring::stringify(vec![
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
            ]))
            .send()
            .await
            .map_err(SpotifyApiError::from)?;

        return Ok(response
            .json::<SpotifyTokenResponse>()
            .await
            .map_err(SpotifyApiError::from)?);
    }

    async fn get_playlist_tracks(
        &self,
        token: String,
        playlist_id: String
    ) -> SpotifyApiResult<Vec<SpotifyTrack>> {
        return log(format!("Get tracks from playlist {}", playlist_id), || async {
            let response = get(format!("https://api.spotify.com/v1/playlists/{}/tracks", playlist_id), token).await?
                .json::<SpotifyPlaylistResponse>()
                .await
                .map_err(SpotifyApiError::from)?;

            return Ok(response.items.iter().map(|item| item.track.clone()).collect());
        }).await;
    }

    async fn get_playback_state(
        &self,
        token: String
    ) -> SpotifyApiResult<Option<SpotifyPlaybackState>> {
        return log("Get playback state".to_string(), || async {
            let response = get("https://api.spotify.com/v1/me/player".to_string(), token).await?;
            if response.status() == StatusCode::NO_CONTENT {
                return Ok(None);
            } else {
                return response
                    .json::<Option<SpotifyPlaybackState>>()
                    .await
                    .map_err(SpotifyApiError::from);
            }
        }).await;
    }
}

fn prepare_headers(client_id: &String, client_secret: &String) -> HeaderMap {
    let base64_authorization = encode(format!("{}:{}", client_id, client_secret));
    let mut headers = HeaderMap::new();
    headers.insert("Authorization", format!("Basic {}", base64_authorization).parse().unwrap());
    headers.insert("Content-Type", "application/x-www-form-urlencoded".parse().unwrap());
    return headers;
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

async fn get(url: String, token: String) -> SpotifyApiResult<Response> {
    let client = Client::new();
    let response = client.get(url)
        .headers(headers(token))
        .send()
        .await
        .map_err(SpotifyApiError::from)?;

    if response.status() == StatusCode::UNAUTHORIZED {
        return Err(SpotifyApiError::Unauthorized);
    } else {
        return Ok(response);
    }
}

fn headers(token: String) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("Authorization", format!("Bearer {}", token).parse().unwrap());
    return headers;
}
