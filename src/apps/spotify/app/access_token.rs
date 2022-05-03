use std::future::Future;
use std::sync::Arc;

use crate::apps::spotify::config::Config; 
use crate::apps::spotify::client::{SpotifyApiError, SpotifyApiResult, SpotifyTokenResponse}; 

use super::app::*;

pub async fn with_access_token<A, F, Fut>(config: Arc<Config>, state: Arc<State>, f: F) -> SpotifyApiResult<A> where
    F: Fn(String) -> Fut,
    Fut: Future<Output = SpotifyApiResult<A>>,
{
    let token = state.access_token.lock().unwrap().clone();
    return match token {
        Some(token) => {
            println!("[Spotify] Found token in memory");
            match f(token.to_string()).await {
                Err(SpotifyApiError::Unauthorized) => {
                    println!("[Spotify] Retrying because of expired token");
                    let token = fetch_and_store_access_token(config, state).await?;
                    return f(token).await;
                },
                Err(err) => Err(err),
                Ok(a) => Ok(a),
            }
        },
        None => {
            println!("[Spotify] No token in memory");
            let token = fetch_and_store_access_token(config, state).await?;
            return f(token).await;
        },
    };
}

async fn fetch_and_store_access_token(config: Arc<Config>, state: Arc<State>) ->  SpotifyApiResult<String> {
    let token_response =  state.client.refresh_token(
        &config.client_id,
        &config.client_secret,
        &config.refresh_token
    ).await?;

    let mut new_token = state.access_token.lock().unwrap();
    *new_token = Some(token_response.access_token.clone());
    return Ok(token_response.access_token);
}

#[cfg(test)]
mod test {
    use std::sync::Mutex;
    use std::time::Instant;

    use mockall::predicate::*;
    use tokio::runtime::Builder;

    use crate::apps::spotify::client::MockSpotifyApiClient;
    use super::*;

    #[test]
    fn with_access_token_when_valid_token_in_state_then_do_not_refresh_token() {
        let config = Arc::new(Config {
            playlist_id: "playlist_id".to_string(),
            client_id: "client_id".to_string(),
            client_secret: "client_secret".to_string(),
            refresh_token: "refresh_token".to_string(),
        });

        let mut client = MockSpotifyApiClient::new();
        client.expect_refresh_token().times(0);

        let state = Arc::new(State {
            client: Box::new(client),
            input_transformer: crate::midi::devices::default::transformer(),
            output_transformer: crate::midi::devices::default::transformer(),
            access_token: Mutex::new(Some("access_token".to_string())),
            last_action: Mutex::new(Instant::now()),
            tracks: Mutex::new(Some(vec![])),
            playing: Mutex::new(None),
        });

        Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(async move {
                let result = with_access_token(config, state, |token| async {
                    let token = token;
                    assert_eq!(token, "access_token".to_string());
                    Ok(())
                }).await;

                assert!(result.is_ok());
            });
    }

    #[test]
    fn with_access_token_when_no_token_in_state_then_do_refresh_token() {
        let config = Arc::new(Config {
            playlist_id: "playlist_id".to_string(),
            client_id: "client_id".to_string(),
            client_secret: "client_secret".to_string(),
            refresh_token: "refresh_token".to_string(),
        });

        let mut client = MockSpotifyApiClient::new();
        client.expect_refresh_token()
            .times(1)
            .with(eq("client_id".to_string()), eq("client_secret".to_string()), eq("refresh_token".to_string()))
            .returning(|_, _, _| Ok(SpotifyTokenResponse {
                access_token: "access_token".to_string(),
                token_type: "bearer".to_string(),
                expires_in: 3600,
                scope: Some("scope".to_string()),
                refresh_token: Some("refresh_token".to_string()),
            }));

        let state = Arc::new(State {
            client: Box::new(client),
            input_transformer: crate::midi::devices::default::transformer(),
            output_transformer: crate::midi::devices::default::transformer(),
            access_token: Mutex::new(None),
            last_action: Mutex::new(Instant::now()),
            tracks: Mutex::new(Some(vec![])),
            playing: Mutex::new(None),
        });

        Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(async move {
                let result = with_access_token(config, state, |token| async {
                    let token = token;
                    assert_eq!(token, "access_token".to_string());
                    Ok(())
                }).await;

                assert!(result.is_ok());
            });
    }

    #[test]
    fn with_access_token_when_token_in_state_resulting_in_unauthorized_then_do_refresh_token() {
        let config = Arc::new(Config {
            playlist_id: "playlist_id".to_string(),
            client_id: "client_id".to_string(),
            client_secret: "client_secret".to_string(),
            refresh_token: "refresh_token".to_string(),
        });

        let mut client = MockSpotifyApiClient::new();
        client.expect_refresh_token()
            .times(1)
            .with(eq("client_id".to_string()), eq("client_secret".to_string()), eq("refresh_token".to_string()))
            .returning(|_, _, _| Ok(SpotifyTokenResponse {
                access_token: "fresh_access_token".to_string(),
                token_type: "bearer".to_string(),
                expires_in: 3600,
                scope: Some("scope".to_string()),
                refresh_token: Some("refresh_token".to_string()),
            }));

        let state = Arc::new(State {
            client: Box::new(client),
            input_transformer: crate::midi::devices::default::transformer(),
            output_transformer: crate::midi::devices::default::transformer(),
            access_token: Mutex::new(Some("expired_access_token".to_string())),
            last_action: Mutex::new(Instant::now()),
            tracks: Mutex::new(Some(vec![])),
            playing: Mutex::new(None),
        });

        let tokens = Arc::new(Mutex::new(vec![]));
        let thread_tokens = Arc::clone(&tokens);
        Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(async move {
                let thread_tokens = Arc::clone(&thread_tokens);
                let result = with_access_token(config, state, |token| async {
                    let token = token;
                    let mut tokens = thread_tokens.lock().unwrap();
                    tokens.push(token.clone());

                    if token == "expired_access_token".to_string() {
                        Err(SpotifyApiError::Unauthorized)
                    } else {
                        Ok(())
                    }
                }).await;

                assert!(result.is_ok());
            });

        let tokens = tokens.lock().unwrap();
        assert_eq!(*tokens, ["expired_access_token", "fresh_access_token"]);
    }

    #[test]
    fn with_access_token_when_valid_token_in_state_and_callback_failed_then_return_error() {
        let config = Arc::new(Config {
            playlist_id: "playlist_id".to_string(),
            client_id: "client_id".to_string(),
            client_secret: "client_secret".to_string(),
            refresh_token: "refresh_token".to_string(),
        });

        let mut client = MockSpotifyApiClient::new();
        client.expect_refresh_token().times(0);

        let state = Arc::new(State {
            client: Box::new(client),
            input_transformer: crate::midi::devices::default::transformer(),
            output_transformer: crate::midi::devices::default::transformer(),
            access_token: Mutex::new(Some("fresh_access_token".to_string())),
            last_action: Mutex::new(Instant::now()),
            tracks: Mutex::new(Some(vec![])),
            playing: Mutex::new(None),
        });

        Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(async move {
                let result = with_access_token(config, state, |token| async {
                    let token = token;
                    assert_eq!(token.clone(), "fresh_access_token".clone());

                    let result: SpotifyApiResult<String> = Err(
                        SpotifyApiError::Other(Box::new(std::io::Error::from(std::io::ErrorKind::NotConnected)))
                    );
                    result
                }).await;

                assert!(result.is_err());
            });
    }
}
