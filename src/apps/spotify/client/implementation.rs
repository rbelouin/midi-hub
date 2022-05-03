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

pub const SPOTIFY_API_CLIENT: SpotifyApiClientImpl = SpotifyApiClientImpl {};

pub struct SpotifyApiClientImpl {}

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
                let token = SPOTIFY_API_CLIENT.refresh_token(
                    &client_id,
                    &client_secret,
                    &refresh_token,
                ).await.unwrap();

                let playlist_tracks = SPOTIFY_API_CLIENT
                    .get_playlist_tracks(token.access_token.clone(), "1vsF6HQZWDv6BHPPBevJMG".to_string())
                    .await
                    .unwrap();

                assert_eq!(playlist_tracks.len(), 64, "The playlist under test should have 64 tracks");

                let playback_state = SPOTIFY_API_CLIENT
                    .get_playback_state(token.access_token.clone())
                    .await
                    .unwrap();

                // Hopefully, no one is using the test account during the test execution!
                assert_eq!(playback_state, None, "No tracks should be playing at the moment");
            });
    }
}
