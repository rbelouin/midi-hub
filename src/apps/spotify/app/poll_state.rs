use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::apps::spotify::config::Config;
use crate::apps::spotify::client::SpotifyApiResult;
use super::app::State;

use super::access_token::with_access_token;

pub async fn poll_state(
    config: Arc<Config>,
    state: Arc<State>,
    terminate: Arc<AtomicBool>,
) {
    while terminate.load(Ordering::Relaxed) != true {
        match get_currently_playing_index(Arc::clone(&config), Arc::clone(&state)).await {
            Ok(currently_playing) => {
                let previously_playing = state.playing.lock().unwrap().clone();

                if previously_playing != currently_playing {
                    {
                        let mut playing = state.playing.lock().unwrap();
                        *playing = currently_playing;
                    }
                }
            },
            Err(err) => eprintln!("[spotify] could not poll playback state: {}", err),
        }

        std::thread::sleep(Duration::from_millis(1_000));
    }
}

async fn get_currently_playing_index(config: Arc<Config>, state: Arc<State>) -> SpotifyApiResult<Option<u16>> {
    with_access_token(config, Arc::clone(&state), |token| async {
        let playback_state = state.client.get_playback_state(token).await?;

        return Ok(playback_state
            .filter(|playback_state| playback_state.is_playing)
            .and_then(|playback_state| {
                let tracks = state.tracks.lock().unwrap();
                if let Some(tracks) = tracks.as_ref() {
                    for i in 0..tracks.len() {
                        if tracks[i].id == playback_state.item.id {
                            return Some(i as u16);
                        }
                    }
                }
                None
            })
        );
    }).await
}

#[cfg(test)]
mod test {
    use std::time::Instant;
    use std::sync::Mutex;

    use mockall::predicate::*;
    use tokio::runtime::Builder;

    use crate::apps::spotify::client::{
        MockSpotifyApiClient,
        SpotifyAlbum,
        SpotifyAlbumImage,
        SpotifyPlaybackState,
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
    fn test_poll_state_call_once_per_second() {
        let mut client = Box::new(MockSpotifyApiClient::new());
        client.expect_refresh_token().times(0);
        client.expect_get_playback_state()
            .times(3)
            .with(eq("access_token".to_string()))
            .returning(|_| Ok(None));

        let config = Arc::new(Config {
            playlist_id: "playlist_id".to_string(),
            client_id: "client_id".to_string(),
            client_secret: "client_secret".to_string(),
            refresh_token: "refresh_token".to_string(),
        });

        let state = Arc::new(State {
            client,
            input_transformer: crate::midi::devices::default::transformer(),
            output_transformer: crate::midi::devices::default::transformer(),
            access_token: Mutex::new(Some("access_token".to_string())),
            last_action: Mutex::new(Instant::now()),
            tracks: Mutex::new(Some(vec![lingus(), conscious_club()])),
            playing: Mutex::new(None),
        });

        Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(async move {
                let terminate = Arc::new(AtomicBool::new(false));

                let terminate_copy = Arc::clone(&terminate);
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(2_500));
                    terminate_copy.store(true, Ordering::Relaxed);
                });

                poll_state(
                    Arc::clone(&config),
                    Arc::clone(&state),
                    terminate,
                ).await;
            });
    }

    #[test]
    fn test_poll_state_when_nothing_is_playing_then_do_nothing() {
        let mut client = Box::new(MockSpotifyApiClient::new());
        client.expect_refresh_token().times(0);
        client.expect_get_playback_state()
            .times(1)
            .with(eq("access_token".to_string()))
            .returning(|_| Ok(None));

        let config = Arc::new(Config {
            playlist_id: "playlist_id".to_string(),
            client_id: "client_id".to_string(),
            client_secret: "client_secret".to_string(),
            refresh_token: "refresh_token".to_string(),
        });

        let state = Arc::new(State {
            client,
            input_transformer: crate::midi::devices::default::transformer(),
            output_transformer: crate::midi::devices::default::transformer(),
            access_token: Mutex::new(Some("access_token".to_string())),
            last_action: Mutex::new(Instant::now()),
            tracks: Mutex::new(Some(vec![lingus(), conscious_club()])),
            playing: Mutex::new(None),
        });

        Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(async move {
                let terminate = Arc::new(AtomicBool::new(false));

                let terminate_copy = Arc::clone(&terminate);
                std::thread::spawn(move || {
                    terminate_copy.store(true, Ordering::Relaxed);
                });

                poll_state(
                    Arc::clone(&config),
                    Arc::clone(&state),
                    terminate,
                ).await;
            });
    }

    #[test]
    fn test_poll_state_when_starts_playing_then_update_state() {
        let mut client = Box::new(MockSpotifyApiClient::new());
        client.expect_refresh_token().times(0);

        // Returns nothing the first time it’s called
        client.expect_get_playback_state()
            .times(1)
            .with(eq("access_token".to_string()))
            .returning(|_| Ok(None));

        // Returns a track the second time
        client.expect_get_playback_state()
            .times(1)
            .with(eq("access_token".to_string()))
            .returning(|_| Ok(Some(SpotifyPlaybackState {
                is_playing: true,
                item: conscious_club(),
            })));

        let config = Arc::new(Config {
            playlist_id: "playlist_id".to_string(),
            client_id: "client_id".to_string(),
            client_secret: "client_secret".to_string(),
            refresh_token: "refresh_token".to_string(),
        });

        let state = Arc::new(State {
            client,
            input_transformer: crate::midi::devices::default::transformer(),
            output_transformer: crate::midi::devices::default::transformer(),
            access_token: Mutex::new(Some("access_token".to_string())),
            last_action: Mutex::new(Instant::now()),
            tracks: Mutex::new(Some(vec![lingus(), conscious_club()])),
            playing: Mutex::new(None),
        });

        Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(async move {
                let terminate = Arc::new(AtomicBool::new(false));

                let terminate_copy = Arc::clone(&terminate);
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(1_500));
                    terminate_copy.store(true, Ordering::Relaxed);
                });

                poll_state(
                    Arc::clone(&config),
                    Arc::clone(&state),
                    terminate,
                ).await;
            });
    }

    #[test]
    fn test_poll_state_when_stops_playing_then_update_state() {
        let mut client = Box::new(MockSpotifyApiClient::new());
        client.expect_refresh_token().times(0);

        // Returns Lingus the two first times it’s called
        client.expect_get_playback_state()
            .times(2)
            .with(eq("access_token".to_string()))
            .returning(|_| Ok(Some(SpotifyPlaybackState {
                is_playing: true,
                item: lingus(),
            })));

        // Returns a nothing the third time
        client.expect_get_playback_state()
            .times(1)
            .with(eq("access_token".to_string()))
            .returning(|_| Ok(None));

        let config = Arc::new(Config {
            playlist_id: "playlist_id".to_string(),
            client_id: "client_id".to_string(),
            client_secret: "client_secret".to_string(),
            refresh_token: "refresh_token".to_string(),
        });

        let state = Arc::new(State {
            client,
            input_transformer: crate::midi::devices::default::transformer(),
            output_transformer: crate::midi::devices::default::transformer(),
            access_token: Mutex::new(Some("access_token".to_string())),
            last_action: Mutex::new(Instant::now()),
            tracks: Mutex::new(Some(vec![lingus(), conscious_club()])),
            playing: Mutex::new(Some(0)),
        });

        Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(async move {
                let terminate = Arc::new(AtomicBool::new(false));

                let terminate_copy = Arc::clone(&terminate);
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(2_500));
                    terminate_copy.store(true, Ordering::Relaxed);
                });

                poll_state(
                    Arc::clone(&config),
                    Arc::clone(&state),
                    terminate,
                ).await;
            });
    }

    #[test]
    fn test_poll_state_when_pauses_playing_then_update_state() {
        let mut client = Box::new(MockSpotifyApiClient::new());
        client.expect_refresh_token().times(0);

        // Returns Lingus the two first times it’s called
        client.expect_get_playback_state()
            .times(2)
            .with(eq("access_token".to_string()))
            .returning(|_| Ok(Some(SpotifyPlaybackState {
                is_playing: true,
                item: lingus(),
            })));

        // Returns a paused Lingus the third time
        client.expect_get_playback_state()
            .times(1)
            .with(eq("access_token".to_string()))
            .returning(|_| Ok(Some(SpotifyPlaybackState {
                is_playing: false,
                item: lingus(),
            })));

        let config = Arc::new(Config {
            playlist_id: "playlist_id".to_string(),
            client_id: "client_id".to_string(),
            client_secret: "client_secret".to_string(),
            refresh_token: "refresh_token".to_string(),
        });

        let state = Arc::new(State {
            client,
            input_transformer: crate::midi::devices::default::transformer(),
            output_transformer: crate::midi::devices::default::transformer(),
            access_token: Mutex::new(Some("access_token".to_string())),
            last_action: Mutex::new(Instant::now()),
            tracks: Mutex::new(Some(vec![lingus(), conscious_club()])),
            playing: Mutex::new(Some(0)),
        });

        Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(async move {
                let terminate = Arc::new(AtomicBool::new(false));

                let terminate_copy = Arc::clone(&terminate);
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(2_500));
                    terminate_copy.store(true, Ordering::Relaxed);
                });

                poll_state(
                    Arc::clone(&config),
                    Arc::clone(&state),
                    terminate,
                ).await;
            });
    }

    #[test]
    fn test_poll_state_when_playing_an_unknown_track_then_treat_it_like_stop() {
        let mut client = Box::new(MockSpotifyApiClient::new());
        client.expect_refresh_token().times(0);

        // Conscious Club is playing
        client.expect_get_playback_state()
            .times(1)
            .with(eq("access_token".to_string()))
            .returning(|_| Ok(Some(SpotifyPlaybackState {
                is_playing: true,
                item: conscious_club(),
            })));

        let config = Arc::new(Config {
            playlist_id: "playlist_id".to_string(),
            client_id: "client_id".to_string(),
            client_secret: "client_secret".to_string(),
            refresh_token: "refresh_token".to_string(),
        });

        let state = Arc::new(State {
            client,
            input_transformer: crate::midi::devices::default::transformer(),
            output_transformer: crate::midi::devices::default::transformer(),
            access_token: Mutex::new(Some("access_token".to_string())),
            last_action: Mutex::new(Instant::now()),
            // But Conscious Club cannot be found in the local library
            tracks: Mutex::new(Some(vec![lingus()])),
            playing: Mutex::new(None),
        });

        Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(async move {
                let terminate = Arc::new(AtomicBool::new(false));

                let terminate_copy = Arc::clone(&terminate);
                std::thread::spawn(move || {
                    terminate_copy.store(true, Ordering::Relaxed);
                });

                poll_state(
                    Arc::clone(&config),
                    Arc::clone(&state),
                    terminate,
                ).await;
            });
    }
}
