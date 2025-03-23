use std::sync::Arc;

use crate::apps::ServerCommand;
use super::app::*;

pub async fn play_or_pause(
    state: Arc<State>,
    index: usize,
) {
    let playback = state.playback.lock().unwrap().clone();
    match playback {
        PlaybackState::PAUSED | PlaybackState::PAUSING => play(state, index).await,
        PlaybackState::REQUESTED(i) | PlaybackState::PLAYING(i) => {
            if i == index {
                pause(state).await
            } else {
                play(state, index).await
            }
        },
    };
}

async fn play(
    state: Arc<State>,
    index: usize,
) {
    // Find the track corresponding to the given index
    let track = state.tracks.lock().unwrap().as_ref()
        .and_then(|tracks| tracks.get(index as usize))
        .map(|track| track.clone());

    return match track {
        Some(track) => {
            let access_token = state.access_token.lock().unwrap()
                .clone()
                .expect("it should not be possible to have tracks in memory without a valid access_token");

            let device_id = state.device_id.lock().unwrap().clone();

            let command = ServerCommand::SpotifyToken {
                access_token: access_token.clone(),
            };

            // Send the token to the web player so that it can render the current track
            state.sender.send(command.into()).await
                .unwrap_or_else(|err| eprintln!("[spotify] could not send token command: {}", err));

            state.client.start_or_resume_playback(access_token, vec![track.uri], device_id).await
                .unwrap_or_else(|err| eprintln!("[spotify] could not send play command: {}", err));

            let mut playback = state.playback.lock().unwrap();
            *playback = PlaybackState::REQUESTED(index);
        },
        _ => {},
    }
}

async fn pause(state: Arc<State>) {
    let access_token = state.access_token.lock().unwrap()
        .clone()
        .expect("it should not be possible to have a playing track without a valid access_token");

    state.client.pause_playback(access_token).await
        .unwrap_or_else(|err| eprintln!("[spotify] could not send pause command: {}", err));

    let mut playback = state.playback.lock().unwrap();
    *playback = PlaybackState::PAUSING;
}

#[cfg(test)]
mod test {
    use std::future::Future;
    use std::time::Instant;
    use std::sync::Mutex;

    use mockall::predicate::*;

    use tokio::runtime::Builder;
    use tokio::sync::mpsc::channel;

    use crate::apps::spotify::config::Config;
    use crate::apps::spotify::client::{MockSpotifyApiClient, SpotifyAlbum, SpotifyAlbumImage, SpotifyTrack};

    use super::*;
    use super::PlaybackState::{PAUSED, PAUSING, REQUESTED, PLAYING};

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
    fn play_or_pause_when_no_song_playing_then_call_start_or_resume() {
        let mut client = MockSpotifyApiClient::new();
        client.expect_start_or_resume_playback()
            .times(1)
            .with(eq("access_token".to_string()), eq(vec!["spotify:track:5vmFVIJV9XN1l01YsFuKL3".to_string()]), eq(Some("device_id".to_string())))
            .returning(|_, _, _| Ok(()));
        client.expect_pause_playback().never();

        let state = get_state_with_playing_and_client(PAUSED, client);

        with_runtime(async move {
            play_or_pause(Arc::clone(&state), 1).await;
        });
    }

    #[test]
    fn play_or_pause_when_no_song_playing_and_index_out_of_bound_then_ignore() {
        let mut client = MockSpotifyApiClient::new();
        client.expect_start_or_resume_playback().never();
        client.expect_pause_playback().never();

        let state = get_state_with_playing_and_client(PAUSING, client);

        with_runtime(async move {
            play_or_pause(Arc::clone(&state), 24).await;
        });
    }

    #[test]
    fn play_or_pause_when_index_matches_song_currently_playing_then_call_pause() {
        let mut client = MockSpotifyApiClient::new();
        client.expect_start_or_resume_playback().never();
        client.expect_pause_playback()
            .times(1)
            .with(eq("access_token".to_string()))
            .returning(|_| Ok(()));

        let state = get_state_with_playing_and_client(PLAYING(1), client);

        with_runtime(async move {
            play_or_pause(Arc::clone(&state), 1).await;
        });
    }

    #[test]
    fn play_or_pause_when_index_does_not_match_song_currently_playing_then_call_play_or_resume() {
        let mut client = MockSpotifyApiClient::new();
        client.expect_start_or_resume_playback()
            .times(1)
            .with(eq("access_token".to_string()), eq(vec!["spotify:track:68d6ZfyMUYURol2y15Ta2Y".to_string()]), eq(Some("device_id".to_string())))
            .returning(|_, _, _| Ok(()));
        client.expect_pause_playback().never();

        let state = get_state_with_playing_and_client(REQUESTED(1), client);

        with_runtime(async move {
            play_or_pause(Arc::clone(&state), 0).await;
        });
    }

    #[test]
    fn play_or_pause_when_song_playing_and_index_out_of_bound_then_ignore() {
        let mut client = MockSpotifyApiClient::new();
        client.expect_start_or_resume_playback().never();
        client.expect_pause_playback().never();

        let state = get_state_with_playing_and_client(PLAYING(0), client);

        with_runtime(async move {
            play_or_pause(Arc::clone(&state), 24).await;
        });
    }

    fn get_state_with_playing_and_client(playback: PlaybackState, client: MockSpotifyApiClient) -> Arc<State> {
        let (sender, _) = channel::<Out>(32);
        let config = Config {
            playlist_id: "playlist_id".to_string(),
            client_id: "client_id".to_string(),
            client_secret: "client_secret".to_string(),
            refresh_token: "refresh_token".to_string(),
        };

        Arc::new(State {
            client: Box::new(client),
            input_features: Arc::new(crate::midi::devices::default::DefaultFeatures::new()),
            output_features: Arc::new(crate::midi::devices::default::DefaultFeatures::new()),
            access_token: Mutex::new(Some("access_token".to_string())),
            last_action: Mutex::new(Instant::now()),
            tracks: Mutex::new(Some(vec![lingus(), conscious_club()])),
            playback: Mutex::new(playback),
            device_id: Mutex::new(Some("device_id".to_string())),
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
