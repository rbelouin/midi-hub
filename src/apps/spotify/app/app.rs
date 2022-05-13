use tokio::runtime::Builder;
use tokio::sync::mpsc;

use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use crate::apps::App;
use crate::image::Image;
use crate::midi::EventTransformer;

use super::super::config::Config;
use super::super::client::*;

use super::playback::*;
use super::poll_events::*;
use super::poll_state::*;
use super::poll_playlist::*;
use super::render_state::*;

pub const NAME: &'static str = "spotify";
pub const COLOR: [u8; 3] = [0, 255, 0];

pub const DELAY: Duration = Duration::from_millis(5_000);
pub const PLAYLIST_POLLING_INTERVAL: Duration = Duration::from_secs(600);

pub type In = crate::apps::In;
pub type Out = crate::apps::Out;
pub type Sender<T> = tokio::sync::mpsc::Sender<T>;
pub type Receiver<T> = tokio::sync::mpsc::Receiver<T>;

pub struct State {
    pub client: Box<dyn SpotifyApiClient + Send + Sync>,
    pub input_transformer: &'static (dyn EventTransformer + Sync),
    pub output_transformer: &'static (dyn EventTransformer + Sync),
    pub access_token: Mutex<Option<String>>,
    pub last_action: Mutex<Instant>,
    pub tracks: Mutex<Option<Vec<SpotifyTrack>>>,
    pub playback: Mutex<PlaybackState>,
    pub config: Config,
    pub sender: Sender<Out>,
}

#[derive(Clone, Debug)]
pub enum PlaybackState {
    PAUSED,
    PAUSING,
    REQUESTED(u16),
    PLAYING(u16),
}

pub struct Spotify {
    in_sender: Sender<In>,
    out_receiver: Receiver<Out>,
}

impl Spotify {
    pub fn new(
        config: Config,
        client: Box<dyn SpotifyApiClient + Send + Sync>,
        input_transformer: &'static (dyn EventTransformer + Sync),
        output_transformer: &'static (dyn EventTransformer + Sync),
    ) -> Self {
        let (in_sender, in_receiver) = mpsc::channel::<In>(32);
        let (out_sender, out_receiver) = mpsc::channel::<Out>(32);

        let state = Arc::new(State {
            client,
            input_transformer,
            output_transformer,
            access_token: Mutex::new(None),
            last_action: Mutex::new(Instant::now() - DELAY),
            tracks: Mutex::new(None),
            playback: Mutex::new(PlaybackState::PAUSED),
            config,
            sender: out_sender,
        });

        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        std::thread::spawn(move || {
            runtime.block_on(async move {
                let poll_playlist_state = Arc::clone(&state);
                tokio::spawn(async move {
                    poll_playlist(
                        poll_playlist_state,
                        PLAYLIST_POLLING_INTERVAL,
                        Arc::new(AtomicBool::new(false)),
                    ).await;
                });

                let poll_state_state = Arc::clone(&state);
                tokio::spawn(async move {
                    poll_state(
                        poll_state_state,
                        Arc::new(AtomicBool::new(false)),
                    ).await;
                });

                let render_state_state = Arc::clone(&state);
                tokio::spawn(async move {
                    render_state_reactively(
                        render_state_state,
                        Arc::new(AtomicBool::new(false)),
                    ).await;
                });

                let poll_events_state = Arc::clone(&state);
                poll_events(poll_events_state, in_receiver, play_or_pause).await;
            });
        });

        let spotify = Spotify {
            in_sender,
            out_receiver,
        };

        return spotify;
    }
}

impl App for Spotify {
    fn get_name(&self) -> &'static str {
        return NAME;
    }

    fn get_color(&self) -> [u8; 3] {
        return COLOR;
    }

    fn get_logo(&self) -> Image {
        return get_logo();
    }

    fn send(&mut self, event: In) -> Result<(), mpsc::error::SendError<In>> {
        return self.in_sender.blocking_send(event);
    }

    fn receive(&mut self) -> Result<Out, mpsc::error::TryRecvError> {
        return self.out_receiver.try_recv();
    }

    fn on_select(&mut self) {}
}
