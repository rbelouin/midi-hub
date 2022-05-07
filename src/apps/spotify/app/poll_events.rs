use std::future::Future;
use std::sync::Arc;
use std::time::Instant;

use super::app::*;

pub async fn poll_events<F, Fut>(
    state: Arc<State>,
    mut in_receiver: Receiver<In>,
    play_or_pause: F,
) where
    F: Fn(Arc<State>, u16) -> Fut + Copy,
    Fut: Future<Output = ()>,
{
    while let Some(event) = in_receiver.recv().await {
        let time_elapsed = Arc::clone(&state).last_action.lock().unwrap().elapsed();
        if time_elapsed > DELAY {
            handle_event(Arc::clone(&state), play_or_pause, event).await;
        } else {
            println!("[spotify] ignoring event: {:?}: {:?}ms", event, time_elapsed);
        }
    }
}

async fn handle_event<F, Fut>(state: Arc<State>, play_or_pause: F, event: In) where
    F: Fn(Arc<State>, u16) -> Fut,
    Fut: Future<Output = ()>,
{
    match event {
        In::Midi(event) => {
            match state.input_transformer.into_index(event) {
                Ok(Some(index)) => {
                    track_last_action(Arc::clone(&state));
                    play_or_pause(Arc::clone(&state), index).await;
                },
                _ => {},
            }
        },
        _ => {},
    }
}

fn track_last_action(state: Arc<State>) {
    let mut last_action = state.last_action.lock().unwrap();
    *last_action = Instant::now();
}

#[cfg(test)]
mod test {
    use std::sync::Mutex;
    use std::time::Duration;

    use tokio::runtime::Builder;
    use tokio::sync::mpsc::error::TryRecvError;

    use crate::apps::{MidiEvent, ServerCommand};
    use crate::apps::spotify::config::Config;
    use crate::apps::spotify::client::MockSpotifyApiClient;
    use super::*;

    #[test]
    fn poll_events_when_valid_event_then_trigger_playback() {
        let (in_sender, in_receiver) = tokio::sync::mpsc::channel::<In>(32);
        let (out_sender, mut out_receiver) = tokio::sync::mpsc::channel::<Out>(32);
        let state = get_state_with_last_action_and_sender(
            Instant::now() - Duration::from_millis(5_000),
            out_sender,
        );

        async fn play_or_pause(state: Arc<State>, _: u16) {
            state.sender.send(Out::Server(ServerCommand::SpotifyPlay {
                track_id: "spotify:track:68d6ZfyMUYURol2y15Ta2Y".to_string(),
                access_token: "access_token".to_string(),
            })).await.unwrap();
        }

        with_runtime(async move {
            std::thread::spawn(move || {
                in_sender.blocking_send(In::Midi(MidiEvent::Midi([144, 36, 100, 0]))).unwrap();
            });

            poll_events(
                Arc::clone(&state),
                in_receiver,
                play_or_pause,
            ).await;
        });

        let event = out_receiver.try_recv();
        assert_eq!(event, Ok(Out::Server(ServerCommand::SpotifyPlay {
            track_id: "spotify:track:68d6ZfyMUYURol2y15Ta2Y".to_string(),
            access_token: "access_token".to_string(),
        })));

        let event = out_receiver.try_recv();
        assert_eq!(event, Err(TryRecvError::Disconnected));
    }

    #[test]
    fn poll_events_when_invalid_event_then_do_nothing() {
        let (in_sender, in_receiver) = tokio::sync::mpsc::channel::<In>(32);
        let (out_sender, mut out_receiver) = tokio::sync::mpsc::channel::<Out>(32);
        let state = get_state_with_last_action_and_sender(
            Instant::now() - Duration::from_millis(5_000),
            out_sender,
        );

        async fn play_or_pause(state: Arc<State>, _: u16) {
            state.sender.send(Out::Server(ServerCommand::SpotifyPlay {
                track_id: "spotify:track:68d6ZfyMUYURol2y15Ta2Y".to_string(),
                access_token: "access_token".to_string(),
            })).await.unwrap();
        }

        with_runtime(async move {
            std::thread::spawn(move || {
                // the first element of the array must be 144 for a default device
                in_sender.blocking_send(In::Midi(MidiEvent::Midi([143, 36, 100, 0]))).unwrap();
            });

            poll_events(
                Arc::clone(&state),
                in_receiver,
                play_or_pause,
            ).await;
        });

        let event = out_receiver.try_recv();
        assert_eq!(event, Err(TryRecvError::Disconnected));
    }

    #[test]
    fn poll_events_when_valid_event_but_last_action_too_recent_then_ignore() {
        let (in_sender, in_receiver) = tokio::sync::mpsc::channel::<In>(32);
        let (out_sender, mut out_receiver) = tokio::sync::mpsc::channel::<Out>(32);
        let state = get_state_with_last_action_and_sender(Instant::now(), out_sender);

        async fn play_or_pause(state: Arc<State>, _: u16) {
            state.sender.send(Out::Server(ServerCommand::SpotifyPlay {
                track_id: "spotify:track:68d6ZfyMUYURol2y15Ta2Y".to_string(),
                access_token: "access_token".to_string(),
            })).await.unwrap();
        }

        with_runtime(async move {
            std::thread::spawn(move || {
                in_sender.blocking_send(In::Midi(MidiEvent::Midi([144, 36, 100, 0]))).unwrap();
            });

            poll_events(
                Arc::clone(&state),
                in_receiver,
                play_or_pause,
            ).await;
        });

        let event = out_receiver.try_recv();
        assert_eq!(event, Err(TryRecvError::Disconnected));
    }

    #[test]
    fn poll_events_when_valid_events_then_throttle() {
        let (in_sender, in_receiver) = tokio::sync::mpsc::channel::<In>(32);
        let (out_sender, mut out_receiver) = tokio::sync::mpsc::channel::<Out>(32);
        let state = get_state_with_last_action_and_sender(Instant::now() - Duration::from_millis(5_000), out_sender);

        async fn play_or_pause(state: Arc<State>, index: u16) {
            state.sender.send(Out::Server(ServerCommand::SpotifyPlay {
                track_id: format!("spotify:track:{}", index),
                access_token: "access_token".to_string(),
            })).await.unwrap();
        }

        with_runtime(async move {
            std::thread::spawn(move || {
                // Not skipped, this is the initial event
                in_sender.blocking_send(In::Midi(MidiEvent::Midi([144, 36, 100, 0]))).unwrap();
                std::thread::sleep(Duration::from_millis(3_000));

                // Skipped, happens only 3s after the initial event
                in_sender.blocking_send(In::Midi(MidiEvent::Midi([144, 37, 100, 0]))).unwrap();
                std::thread::sleep(Duration::from_millis(3_000));

                // Not skipped, it occurs 6s after the initial event
                in_sender.blocking_send(In::Midi(MidiEvent::Midi([144, 38, 100, 0]))).unwrap();
            });

            poll_events(
                Arc::clone(&state),
                in_receiver,
                play_or_pause,
            ).await;
        });

        let event = out_receiver.try_recv();
        assert_eq!(event, Ok(Out::Server(ServerCommand::SpotifyPlay {
            track_id: "spotify:track:0".to_string(),
            access_token: "access_token".to_string(),
        })));

        let event = out_receiver.try_recv();
        assert_eq!(event, Ok(Out::Server(ServerCommand::SpotifyPlay {
            track_id: "spotify:track:2".to_string(),
            access_token: "access_token".to_string(),
        })));

        let event = out_receiver.try_recv();
        assert_eq!(event, Err(TryRecvError::Disconnected));
    }

    fn get_state_with_last_action_and_sender(last_action: Instant, sender: Sender<Out>) -> Arc<State> {
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
            last_action: Mutex::new(last_action),
            tracks: Mutex::new(Some(vec![])),
            playing: Mutex::new(None),
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
