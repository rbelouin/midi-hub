use std::sync::Arc;

use crate::apps::ServerCommand;
use super::app::*;

pub async fn play_or_pause(
    state: Arc<State>,
    index: u16,
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
    index: u16,
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

            let command = ServerCommand::SpotifyPlay {
                track_id: track.uri,
                access_token,
            };

            // Send the command to the web client via the web server
            state.sender.send(command.into()).await
                .unwrap_or_else(|err| eprintln!("[spotify] could not send play command: {}", err));

            let mut playback = state.playback.lock().unwrap();
            *playback = PlaybackState::REQUESTED(index);
        },
        _ => {},
    }
}

async fn pause(state: Arc<State>) {
    // Send the command to the web client via the web server
    state.sender.send(ServerCommand::SpotifyPause.into()).await
        .unwrap_or_else(|err| eprintln!("[spotify] could not send pause command: {}", err));

    let mut playback = state.playback.lock().unwrap();
    *playback = PlaybackState::PAUSING;
}

#[cfg(test)]
mod test {
    use std::future::Future;
    use std::time::Instant;
    use std::sync::Mutex;

    use tokio::runtime::Builder;
    use tokio::sync::mpsc::{Sender, channel};
    use tokio::sync::mpsc::error::TryRecvError;

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
    fn play_or_pause_when_no_song_playing_then_play_song_at_index_and_return_some() {
        let (sender, mut receiver) = channel::<Out>(32);
        let state = get_state_with_playing_and_sender(PAUSED, sender);

        with_runtime(async move {
            play_or_pause(Arc::clone(&state), 1).await;
        });

        let event = receiver.try_recv();
        assert_eq!(event, Ok(Out::Server(ServerCommand::SpotifyPlay {
            track_id: "spotify:track:5vmFVIJV9XN1l01YsFuKL3".to_string(),
            access_token: "access_token".to_string(),
        })));

        let event = receiver.try_recv();
        assert_eq!(event, Err(TryRecvError::Disconnected));
    }

    #[test]
    fn play_or_pause_when_no_song_playing_and_index_out_of_bound_then_ignore_and_return_none() {
        let (sender, mut receiver) = channel::<Out>(32);
        let state = get_state_with_playing_and_sender(PAUSING, sender);

        with_runtime(async move {
            play_or_pause(Arc::clone(&state), 24).await;
        });

        let event = receiver.try_recv();
        assert_eq!(event, Err(TryRecvError::Disconnected));
    }

    #[test]
    fn play_or_pause_when_index_matches_song_currently_playing_then_pause_and_return_none() {
        let (sender, mut receiver) = channel::<Out>(32);
        let state = get_state_with_playing_and_sender(PLAYING(1), sender);

        with_runtime(async move {
            play_or_pause(Arc::clone(&state), 1).await;
        });

        let event = receiver.try_recv();
        assert_eq!(event, Ok(Out::Server(ServerCommand::SpotifyPause)));

        let event = receiver.try_recv();
        assert_eq!(event, Err(TryRecvError::Disconnected));
    }

    #[test]
    fn play_or_pause_when_index_does_not_match_song_currently_playing_then_play_and_return_some() {
        let (sender, mut receiver) = channel::<Out>(32);
        let state = get_state_with_playing_and_sender(REQUESTED(1), sender);

        with_runtime(async move {
            play_or_pause(Arc::clone(&state), 0).await;
        });

        let event = receiver.try_recv();
        assert_eq!(event, Ok(Out::Server(ServerCommand::SpotifyPlay {
            track_id: "spotify:track:68d6ZfyMUYURol2y15Ta2Y".to_string(),
            access_token: "access_token".to_string(),
        })));

        let event = receiver.try_recv();
        assert_eq!(event, Err(TryRecvError::Disconnected));
    }

    #[test]
    fn play_or_pause_when_song_playing_and_index_out_of_bound_then_ignore_and_return_none() {
        let (sender, mut receiver) = channel::<Out>(32);
        let state = get_state_with_playing_and_sender(PLAYING(0), sender);

        with_runtime(async move {
            play_or_pause(Arc::clone(&state), 24).await;
        });

        let event = receiver.try_recv();
        assert_eq!(event, Err(TryRecvError::Disconnected));
    }

    fn get_state_with_playing_and_sender(playback: PlaybackState, sender: Sender<Out>) -> Arc<State> {
        let client = Box::new(MockSpotifyApiClient::new());
        let config = Config {
            playlist_id: "playlist_id".to_string(),
            client_id: "client_id".to_string(),
            client_secret: "client_secret".to_string(),
            refresh_token: "refresh_token".to_string(),
        };

        Arc::new(State {
            client,
            input_transformer: crate::midi::devices::default::transformer(),
            output_transformer: crate::midi::devices::default::transformer(),
            access_token: Mutex::new(Some("access_token".to_string())),
            last_action: Mutex::new(Instant::now()),
            tracks: Mutex::new(Some(vec![lingus(), conscious_club()])),
            playback: Mutex::new(playback),
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
