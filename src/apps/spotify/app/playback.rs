use std::sync::Arc;

use crate::apps::ServerCommand;
use crate::apps::spotify::client::SpotifyTrack;
use super::app::*;

pub async fn play_or_pause(
    state: Arc<State>,
    sender: Arc<Sender<Out>>,
    index: u16,
) -> Option<SpotifyTrack> {
    let playing = *state.playing.lock().unwrap();
    let track = if playing == Some(index) {
        pause(state, sender).await;
        None
    } else {
        play(state, sender, index).await
    };
    return track;
}

async fn play(
    state: Arc<State>,
    sender: Arc<Sender<Out>>,
    index: u16,
) -> Option<SpotifyTrack> {
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
                track_id: track.uri.clone(),
                access_token,
            };

            // Send the command to the web client via the web server
            sender.send(command.into()).await
                .unwrap_or_else(|err| eprintln!("[spotify] could not send play command: {}", err));

            // Update the state optimistically. Just like for the pause action below,
            // if the track does not start playing, the playback event will be polled from
            // Spotify’s Web API in a few seconds anyway.
            let mut playing = state.playing.lock().unwrap();
            *playing = Some(index as u16);

            Some(track)
        },
        _ => None,
    }
}

async fn pause(
    state: Arc<State>,
    sender: Arc<Sender<Out>>,
) {
    // Send the command to the web client via the web server
    sender.send(ServerCommand::SpotifyPause.into()).await
        .unwrap_or_else(|err| eprintln!("[spotify] could not send pause command: {}", err));

    // Update the state, assuming that the track stopped playing successfully.
    // Worst case: we’re wrong and the state will be put back to a valid state in a few sec,
    // after we polled it from the Web API.
    let mut playing = state.playing.lock().unwrap();
    *playing = None;
}

#[cfg(test)]
mod test {
    use std::time::Instant;
    use std::sync::Mutex;

    use tokio::runtime::Builder;
    use tokio::sync::mpsc::channel;

    use crate::apps::spotify::client::{MockSpotifyApiClient, SpotifyAlbum, SpotifyAlbumImage};
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
    fn play_or_pause_when_no_song_playing_then_play_song_at_index_and_return_some() {
        let client = Box::new(MockSpotifyApiClient::new());
        let state = Arc::new(State {
            client,
            input_transformer: crate::midi::devices::default::transformer(),
            output_transformer: crate::midi::devices::default::transformer(),
            access_token: Mutex::new(Some("access_token".to_string())),
            last_action: Mutex::new(Instant::now()),
            tracks: Mutex::new(Some(vec![lingus(), conscious_club()])),
            playing: Mutex::new(None),
        });

        let (sender, mut receiver) = channel::<Out>(32);
        Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(async move {
                let result = play_or_pause(Arc::clone(&state), Arc::new(sender), 1).await;
                assert_eq!(result, Some(conscious_club()));

                let event = receiver.recv().await;
                assert_eq!(event, Some(Out::Server(ServerCommand::SpotifyPlay {
                    track_id: "spotify:track:5vmFVIJV9XN1l01YsFuKL3".to_string(),
                    access_token: "access_token".to_string(),
                })));

                let event = receiver.recv().await;
                assert_eq!(event, None);
            });
    }

    #[test]
    fn play_or_pause_when_no_song_playing_and_index_out_of_bound_then_ignore_and_return_none() {
        let client = Box::new(MockSpotifyApiClient::new());
        let state = Arc::new(State {
            client,
            input_transformer: crate::midi::devices::default::transformer(),
            output_transformer: crate::midi::devices::default::transformer(),
            access_token: Mutex::new(Some("access_token".to_string())),
            last_action: Mutex::new(Instant::now()),
            tracks: Mutex::new(Some(vec![lingus(), conscious_club()])),
            playing: Mutex::new(None),
        });

        let (sender, mut receiver) = channel::<Out>(32);
        Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(async move {
                let result = play_or_pause(Arc::clone(&state), Arc::new(sender), 24).await;
                assert_eq!(result, None);

                let event = receiver.recv().await;
                assert_eq!(event, None);
            });
    }

    #[test]
    fn play_or_pause_when_index_matches_song_currently_playing_then_pause_and_return_none() {
        let client = Box::new(MockSpotifyApiClient::new());
        let state = Arc::new(State {
            client,
            input_transformer: crate::midi::devices::default::transformer(),
            output_transformer: crate::midi::devices::default::transformer(),
            access_token: Mutex::new(Some("access_token".to_string())),
            last_action: Mutex::new(Instant::now()),
            tracks: Mutex::new(Some(vec![lingus(), conscious_club()])),
            playing: Mutex::new(Some(1)),
        });

        let (sender, mut receiver) = channel::<Out>(32);
        Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(async move {
                let result = play_or_pause(Arc::clone(&state), Arc::new(sender), 1).await;
                assert_eq!(result, None);

                let event = receiver.recv().await;
                assert_eq!(event, Some(Out::Server(ServerCommand::SpotifyPause)));

                let event = receiver.recv().await;
                assert_eq!(event, None);
            });
    }

    #[test]
    fn play_or_pause_when_index_does_not_match_song_currently_playing_then_play_and_return_some() {
        let client = Box::new(MockSpotifyApiClient::new());
        let state = Arc::new(State {
            client,
            input_transformer: crate::midi::devices::default::transformer(),
            output_transformer: crate::midi::devices::default::transformer(),
            access_token: Mutex::new(Some("access_token".to_string())),
            last_action: Mutex::new(Instant::now()),
            tracks: Mutex::new(Some(vec![lingus(), conscious_club()])),
            playing: Mutex::new(Some(1)),
        });

        let (sender, mut receiver) = channel::<Out>(32);
        Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(async move {
                let result = play_or_pause(Arc::clone(&state), Arc::new(sender), 0).await;
                assert_eq!(result, Some(lingus()));

                let event = receiver.recv().await;
                assert_eq!(event, Some(Out::Server(ServerCommand::SpotifyPlay {
                    track_id: "spotify:track:68d6ZfyMUYURol2y15Ta2Y".to_string(),
                    access_token: "access_token".to_string(),
                })));

                let event = receiver.recv().await;
                assert_eq!(event, None);
            });
    }

    #[test]
    fn play_or_pause_when_song_playing_and_index_out_of_bound_then_ignore_and_return_none() {
        let client = Box::new(MockSpotifyApiClient::new());
        let state = Arc::new(State {
            client,
            input_transformer: crate::midi::devices::default::transformer(),
            output_transformer: crate::midi::devices::default::transformer(),
            access_token: Mutex::new(Some("access_token".to_string())),
            last_action: Mutex::new(Instant::now()),
            tracks: Mutex::new(Some(vec![lingus(), conscious_club()])),
            playing: Mutex::new(Some(0)),
        });

        let (sender, mut receiver) = channel::<Out>(32);
        Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(async move {
                let result = play_or_pause(Arc::clone(&state), Arc::new(sender), 24).await;
                assert_eq!(result, None);

                let event = receiver.recv().await;
                assert_eq!(event, None);
            });
    }
}
