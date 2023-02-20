use std::collections::HashMap;
use std::future::Future;
use std::marker::Sized;
use std::time::Instant;

use base64::encode;
use reqwest::{Client, Response, StatusCode};
use reqwest::header::HeaderMap;
use serde::Serialize;

use super::*;

impl From<reqwest::Error> for SpotifyApiError {
    fn from(err: reqwest::Error) -> SpotifyApiError {
        return SpotifyApiError::Other(Box::new(err));
    }
}

pub struct SpotifyApiClientImpl {}

impl SpotifyApiClientImpl {
    pub fn new() -> Self {
        return SpotifyApiClientImpl {};
    }
}

#[async_trait]
impl SpotifyApiClient for SpotifyApiClientImpl {
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

    async fn start_or_resume_playback(
        &self,
        token: String,
        uris: Vec<String>,
        device_id: Option<String>,
    ) -> SpotifyApiResult<()> {
        return log(format!("Start or resume playback of {:?}", uris), || async {
            let query = device_id.map(|id| format!("?device_id={}", id)).unwrap_or("".to_string());
            let body = HashMap::from([("uris", uris)]);
            let _ = put(format!("https://api.spotify.com/v1/me/player/play{}", query), token, &body).await?;
            return Ok(());
        }).await;
    }

    async fn pause_playback(
        &self,
        token: String,
    ) -> SpotifyApiResult<()> {
        return log("Pause playback".to_string(), || async {
            let _ = put("https://api.spotify.com/v1/me/player/pause".to_string(), token, "").await?;
            return Ok(());
        }).await;
    }

    async fn get_available_devices(
        &self,
        token: String,
    ) -> SpotifyApiResult<SpotifyDevices> {
        return log("Get available devices".to_string(), || async {
            let response = get("https://api.spotify.com/v1/me/player/devices".to_string(), token).await?;
            return response
                .json::<SpotifyDevices>()
                .await
                .map_err(SpotifyApiError::from);
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

async fn put<P: Serialize + ?Sized>(url: String, token: String, json_body: &P) -> SpotifyApiResult<Response> {
    let client = Client::new();
    let response = client.put(url)
        .headers(headers(token))
        .json(json_body)
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

#[cfg(test)]
mod test {
    use tokio::runtime::Builder;
    use super::*;

    #[test]
    fn integration_test() {
        let client_id = std::env::var("SPOTIFY_CLIENT_ID").expect("SPOTIFY_CLIENT_ID must be set to run this test");
        let client_secret = std::env::var("SPOTIFY_CLIENT_SECRET").expect("SPOTIFY_CLIENT_SECRET must be set to run this test");
        let refresh_token = std::env::var("SPOTIFY_REFRESH_TOKEN").expect("SPOTIFY_REFRESH_TOKEN must be set to run this test");
        Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async move {
                let client = SpotifyApiClientImpl::new();

                let token = client.refresh_token(
                    &client_id,
                    &client_secret,
                    &refresh_token,
                ).await.unwrap();

                let playlist_tracks = client
                    .get_playlist_tracks(token.access_token.clone(), "1vsF6HQZWDv6BHPPBevJMG".to_string())
                    .await
                    .unwrap();

                assert_eq!(playlist_tracks.len(), 64, "The playlist under test should have 64 tracks");

                client
                    .get_playback_state(token.access_token.clone())
                    .await
                    .expect("Playback state should be retrieved successfully");

                client
                    .get_available_devices(token.access_token.clone())
                    .await
                    .expect("Available devices should be retrieved successfully");

                client
                    .start_or_resume_playback(
                        token.access_token.clone(),
                        vec!["spotify:track:7vDtu5DsQEDHag1iJkSkOB".to_string()],
                        None,
                    )
                    .await
                    .expect("Should be able to start or resume playback");

                client
                    .pause_playback(token.access_token.clone())
                    .await
                    .expect("Should be able to pause playback");
            });
    }
}
