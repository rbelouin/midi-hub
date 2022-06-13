use std::future::Future;
use std::sync::Arc;

use crate::apps::spotify::client::{SpotifyApiError, SpotifyApiResult}; 

use super::app::*;

pub async fn with_access_token<A, F, Fut>(state: Arc<State>, f: F) -> SpotifyApiResult<A> where
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
                    let token = fetch_and_store_access_token(state).await?;
                    return f(token).await;
                },
                Err(err) => Err(err),
                Ok(a) => Ok(a),
            }
        },
        None => {
            println!("[Spotify] No token in memory");
            let token = fetch_and_store_access_token(state).await?;
            return f(token).await;
        },
    };
}

async fn fetch_and_store_access_token(state: Arc<State>) ->  SpotifyApiResult<String> {
    let token_response =  state.client.refresh_token(
        &state.config.client_id,
        &state.config.client_secret,
        &state.config.refresh_token
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

    use crate::apps::spotify::config::Config;
    use crate::apps::spotify::client::{MockSpotifyApiClient, SpotifyTokenResponse};

    use super::*;

    #[test]
    fn with_access_token_when_valid_token_in_state_then_do_not_refresh_token() {
        let mut client = MockSpotifyApiClient::new();
        client.expect_refresh_token().times(0);

        let state = get_state_with_token_and_client(Some("access_token"), client);

        with_runtime(async move {
            let result = with_access_token(state, |token| async {
                let token = token;
                assert_eq!(token, "access_token".to_string());
                Ok(())
            }).await;

            assert!(result.is_ok());
        });
    }

    #[test]
    fn with_access_token_when_no_token_in_state_then_do_refresh_token() {
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

        let state = get_state_with_token_and_client(None, client);

        with_runtime(async move {
            let result = with_access_token(state, |token| async {
                let token = token;
                assert_eq!(token, "access_token".to_string());
                Ok(())
            }).await;

            assert!(result.is_ok());
        });
    }

    #[test]
    fn with_access_token_when_token_in_state_resulting_in_unauthorized_then_do_refresh_token() {
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

        let state = get_state_with_token_and_client(Some("expired_access_token"), client);

        let tokens = Arc::new(Mutex::new(vec![]));
        let thread_tokens = Arc::clone(&tokens);
        with_runtime(async move {
            let thread_tokens = Arc::clone(&thread_tokens);
            let result = with_access_token(state, |token| async {
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
        let mut client = MockSpotifyApiClient::new();
        client.expect_refresh_token().times(0);

        let state = get_state_with_token_and_client(Some("fresh_access_token"), client);

        with_runtime(async move {
            let result = with_access_token(state, |token| async {
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

    fn get_state_with_token_and_client(
        initial_access_token: Option<&'static str>,
        mocked_client: MockSpotifyApiClient,
    ) -> Arc<State> {
        let (sender, _) = tokio::sync::mpsc::channel::<Out>(32);

        let config = Config {
            playlist_id: "playlist_id".to_string(),
            client_id: "client_id".to_string(),
            client_secret: "client_secret".to_string(),
            refresh_token: "refresh_token".to_string(),
        };

        Arc::new(State {
            client: Box::new(mocked_client),
            input_features: Arc::new(crate::midi::devices::default::DefaultFeatures::new()),
            output_features: Arc::new(crate::midi::devices::default::DefaultFeatures::new()),
            access_token: Mutex::new(initial_access_token.map(|s| s.into())),
            last_action: Mutex::new(Instant::now()),
            tracks: Mutex::new(None),
            playback: Mutex::new(PlaybackState::PAUSED),
            config,
            sender,
        })
    }

    fn with_runtime<F>(f: F) -> F::Output where F: Future {
        Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(f)
    }
}
