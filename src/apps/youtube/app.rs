use tokio::runtime::Builder;
use tokio::sync::mpsc;

use std::convert::Into;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::apps::{App, Out, ServerCommand};
use crate::image::Image;
use crate::midi::{FromImage, IntoIndex};

use super::Config;
use super::client;

struct State {
    config: Config,
    last_action: Mutex<Instant>,
    items: Mutex<Vec<client::playlist::PlaylistItem>>,
}

pub struct Youtube<E> {
    in_sender: mpsc::Sender<E>,
    out_receiver: mpsc::Receiver<Out<E>>,
}

pub const NAME: &'static str = "youtube";
pub const COLOR: [u8; 3] = [255, 0, 0];

const DELAY: Duration = Duration::from_millis(5_000);

impl<E: 'static> Youtube<E> where
    E: IntoIndex,
    E: FromImage<E>,
    E: Clone,
    E: std::fmt::Debug,
    E: std::marker::Send,
{
    pub fn new(config: Config) -> Self {
        let (in_sender, mut in_receiver) = mpsc::channel::<E>(32);
        let (out_sender, out_receiver) = mpsc::channel::<Out<E>>(32);

        let state = Arc::new(State {
            config,
            last_action: Mutex::new(Instant::now() - DELAY),
            items: Mutex::new(vec![]),
        });

        let rt = Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let state_copy = Arc::clone(&state);
        let out_sender = Arc::new(out_sender);
        std::thread::spawn(move || {
            rt.block_on(async move {
                let _ = render_youtube_logo(Arc::clone(&out_sender)).await;
                let _ = pull_playlist_items(Arc::clone(&state_copy)).await;
                while let Some(event) = in_receiver.recv().await {
                    let state = Arc::clone(&state_copy);
                    let time_elapsed = {
                        let last_action = state.last_action.lock().unwrap();
                        last_action.elapsed()
                    };

                    if time_elapsed > DELAY {
                        tokio::spawn(handle_youtube_task(Arc::clone(&state_copy), Arc::clone(&out_sender), event.clone()));
                    } else {
                        println!("Ignoring event: {:?}", event);
                    }
                }
            });
        });

        Youtube {
            in_sender,
            out_receiver,
        }
    }
}

impl<E: 'static> App<E, Out<E>> for Youtube<E> where
    E: IntoIndex,
    E: FromImage<E>,
    E: Clone,
    E: std::fmt::Debug,
    E: std::marker::Send,
{

    fn get_name(&self) -> &'static str {
        return NAME;
    }

    fn get_color(&self) -> [u8; 3] {
        return COLOR;
    }

    fn get_logo(&self) -> Image {
        return get_logo();
    }

    fn send(&self, event: E) -> Result<(), mpsc::error::SendError<E>> {
        return self.in_sender.blocking_send(event);
    }

    fn receive(&mut self) -> Result<Out<E>, mpsc::error::TryRecvError> {
        return self.out_receiver.try_recv();
    }
}

async fn render_youtube_logo<E>(sender: Arc<mpsc::Sender<Out<E>>>) -> Result<(), ()> where
    E: FromImage<E>,
    E: std::fmt::Debug,
{
    let event = E::from_image(get_logo()).map_err(|err| {
        eprintln!("Could not convert the image into a MIDI event: {:?}", err);
        ()
    })?;

    return sender.send(Out::Event(event)).await.map_err(|err| {
        eprintln!("Could not send the event back to the router: {:?}", err);
        ()
    });
}

pub fn get_logo() -> Image {
    let r = [255, 0, 0];
    let w = [255, 255, 255];

    return Image {
        width: 8,
        height: 8,
        bytes: vec![
            r, r, r, r, r, r, r, r,
            r, r, r, w, r, r, r, r,
            r, r, r, w, w, r, r, r,
            r, r, r, w, w, w, r, r,
            r, r, r, w, w, w, r, r,
            r, r, r, w, w, r, r, r,
            r, r, r, w, r, r, r, r,
            r, r, r, r, r, r, r, r,
        ].concat(),
    };
}

async fn pull_playlist_items(state: Arc<State>) -> Result<(), client::Error> {
    println!("Pulling Youtube playlist items…");
    let new_items = client::playlist::get_all_items(
        state.config.api_key.clone(),
        state.config.playlist_id.clone(),
    ).await?;

    let mut actual_items = state.items.lock().unwrap();
    *actual_items = new_items;
    println!("Pulling Youtube playlist items, done!");
    return Ok(());
}

async fn handle_youtube_task<E>(state: Arc<State>, sender: Arc<mpsc::Sender<Out<E>>>, event_in: E) where
    E: IntoIndex,
    E: std::fmt::Debug,
{
    match event_in.into_index() {
        Ok(Some(index)) => {
            {
                let mut last_action = state.last_action.lock().unwrap();
                *last_action = Instant::now();
            }

            let item = {
                let items = state.items.lock().unwrap();
                items.get(usize::from(index)).map(|item| item.clone())
            };

            match item {
                Some(item) => {
                    let video_id = item.snippet.resource_id.video_id;
                    match sender.send(ServerCommand::YoutubePlay { video_id: video_id.clone() }.into()).await {
                        Ok(_) => println!("Playing track {}", video_id),
                        Err(_) => eprintln!("Could not play track {}", video_id),
                    }
                },
                _ => println!("No track for index: {}", index),
            }
        },
        _ => {},
    };

    let _ = pull_playlist_items(state).await;
}
