use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::app::State;

use super::access_token::with_access_token;

pub async fn poll_playlist(
    state: Arc<State>,
    polling_interval: Duration,
    terminate: Arc<AtomicBool>,
) {
    while terminate.load(Ordering::Relaxed) != true {
        pull_playlist_tracks(Arc::clone(&state)).await;
        tokio::time::sleep(polling_interval).await;
    }
}

async fn pull_playlist_tracks(state: Arc<State>) {
    with_access_token(Arc::clone(&state), |token| async {
        let tracks = state.client.get_playlist_tracks(token, Arc::clone(&state).config.playlist_id.clone()).await?;
        let mut state_tracks = state.tracks.lock().unwrap();
        *state_tracks = Some(tracks);
        Ok(())
    }).await.unwrap_or_else(|err| {
        eprintln!("[spotify] could not pull tracks from playlist {}: {}", state.config.playlist_id, err);
    });
}

#[cfg(test)]
mod test {
    use std::future::Future;
    use std::time::Instant;
    use std::sync::Mutex;

    use mockall::predicate::*;
    use tokio::runtime::Builder;

    use crate::apps::Out;
    use crate::apps::spotify::app::app::PlaybackState;
    use crate::apps::spotify::config::Config;
    use crate::apps::spotify::client::{
        MockSpotifyApiClient,
        SpotifyAlbum,
        SpotifyAlbumImage,
        SpotifyApiError,
        SpotifyTrack
    };

    use super::*;

    fn lingus() -> SpotifyTrack {
        SpotifyTrack {
            name: "We Like It Here".to_string(),
            id: "68d6ZfyMUYURol2y15Ta2Y".to_string(),
            uri: "spotify:track:68d6ZfyMUYURol2y15Ta2Y".to_string(),
            album: SpotifyAlbum {
                images: vec![
                    SpotifyAlbumImage {
                        height: 640,
                        width: 640,
                        url: "https://i.scdn.co/image/ab67616d0000b273a29d1ada28cf3d9d5fe1972d".to_string(),
                    },
                    SpotifyAlbumImage {
                        height: 300,
                        width: 300,
                        url: "https://i.scdn.co/image/ab67616d00001e02a29d1ada28cf3d9d5fe1972d".to_string(),
                    },
                    SpotifyAlbumImage {
                        height: 64,
                        width: 64,
                        url: "https://i.scdn.co/image/ab67616d00004851a29d1ada28cf3d9d5fe1972d".to_string(),
                    },
                ],
            },
        }
    }

    fn conscious_club() -> SpotifyTrack {
        SpotifyTrack {
            name: "Conscious Club".to_string(),
            id: "5vmFVIJV9XN1l01YsFuKL3".to_string(),
            uri: "spotify:track:5vmFVIJV9XN1l01YsFuKL3".to_string(),
            album: SpotifyAlbum {
                images: vec![
                    SpotifyAlbumImage {
                        height: 640,
                        width: 640,
                        url: "https://i.scdn.co/image/ab67616d0000b273325ed53cf3123d2dd3e31556".to_string(),
                    },
                    SpotifyAlbumImage {
                        height: 300,
                        width: 300,
                        url: "https://i.scdn.co/image/ab67616d00001e02325ed53cf3123d2dd3e31556".to_string(),
                    },
                    SpotifyAlbumImage {
                        height: 64,
                        width: 64,
                        url: "https://i.scdn.co/image/ab67616d00004851325ed53cf3123d2dd3e31556".to_string(),
                    },
                ],
            },
        }
    }

    #[test]
    fn test_poll_playlist_when_polling_interval_is_1s_then_poll_3_times_in_2500ms() {
        let mut client = MockSpotifyApiClient::new();
        client.expect_get_playlist_tracks()
            .times(3)
            .with(eq("access_token".to_string()), eq("playlist_id".to_string()))
            .returning(|_, _| Ok(vec![lingus(), conscious_club()]));

        let state = get_state_with_client_and_tracks(client, vec![]);

        with_runtime(async move {
            let terminate = Arc::new(AtomicBool::new(false));

            let terminate_copy = Arc::clone(&terminate);
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(2_500));
                terminate_copy.store(true, Ordering::Relaxed);
            });

            poll_playlist(
                Arc::clone(&state),
                Duration::from_millis(1_000),
                terminate,
            ).await;
        });
    }

    #[test]
    fn test_poll_playlist_when_polling_interval_is_2s_then_poll_2_times_in_2500ms() {
        let mut client = MockSpotifyApiClient::new();
        client.expect_get_playlist_tracks()
            .times(2)
            .with(eq("access_token".to_string()), eq("playlist_id".to_string()))
            .returning(|_, _| Ok(vec![lingus(), conscious_club()]));

        let state = get_state_with_client_and_tracks(client, vec![]);

        with_runtime(async move {
            let terminate = Arc::new(AtomicBool::new(false));

            let terminate_copy = Arc::clone(&terminate);
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(2_500));
                terminate_copy.store(true, Ordering::Relaxed);
            });

            poll_playlist(
                Arc::clone(&state),
                Duration::from_millis(2_000),
                terminate,
            ).await;
        });
    }

    #[test]
    fn test_poll_playlist_when_request_succeeds_then_update_state() {
        let mut client = MockSpotifyApiClient::new();
        client.expect_refresh_token().times(0);
        client.expect_get_playlist_tracks()
            .times(1)
            .with(eq("access_token".to_string()), eq("playlist_id".to_string()))
            .returning(|_, _| Ok(vec![lingus(), conscious_club()]));

        let state = get_state_with_client_and_tracks(client, vec![]);

        let thread_state = Arc::clone(&state);
        with_runtime(async move {
            let terminate = Arc::new(AtomicBool::new(false));

            let terminate_copy = Arc::clone(&terminate);
            std::thread::spawn(move || {
                terminate_copy.store(true, Ordering::Relaxed);
            });

            poll_playlist(
                thread_state,
                Duration::from_millis(100),
                terminate,
            ).await;
        });

        assert_eq!(*state.tracks.lock().unwrap(), Some(vec![lingus(), conscious_club()]));
    }

    #[test]
    fn test_poll_playlist_when_request_fails_then_do_not_update_state() {
        let mut client = MockSpotifyApiClient::new();
        client.expect_refresh_token().times(0);
        client.expect_get_playlist_tracks()
            .times(1)
            .with(eq("access_token".to_string()), eq("playlist_id".to_string()))
            .returning(|_, _| Err(SpotifyApiError::Other(Box::new(std::io::Error::from(std::io::ErrorKind::NotFound)))));

        let state = get_state_with_client_and_tracks(client, vec![lingus(), conscious_club()]);

        let thread_state = Arc::clone(&state);
        with_runtime(async move {
            let terminate = Arc::new(AtomicBool::new(false));

            let terminate_copy = Arc::clone(&terminate);
            std::thread::spawn(move || {
                terminate_copy.store(true, Ordering::Relaxed);
            });

            poll_playlist(
                thread_state,
                Duration::from_millis(100),
                terminate,
            ).await;
        });

        assert_eq!(*state.tracks.lock().unwrap(), Some(vec![lingus(), conscious_club()]));
    }

    fn get_state_with_client_and_tracks(
        mocked_client: MockSpotifyApiClient,
        tracks: Vec<SpotifyTrack>,
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
            access_token: Mutex::new(Some("access_token".to_string())),
            last_action: Mutex::new(Instant::now()),
            tracks: Mutex::new(Some(tracks)),
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
