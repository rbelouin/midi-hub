use tokio::runtime::Builder;
use tokio::sync::mpsc;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::image::Image;
use crate::midi::{FromImage, IntoIndex};

use super::{Command, Out};
use super::client;

#[derive(Clone)]
pub struct Youtube<E> {
    state: Arc<State>,
    spawn: mpsc::Sender<E>,
}

struct State {
    api_key: String,
    playlist_id: String,
    last_action: Mutex<Instant>,
    items: Mutex<Vec<client::playlist::PlaylistItem>>,
}

const DELAY: Duration = Duration::from_millis(5_000);

impl<E: 'static> Youtube<E> {
    pub fn new(api_key: String, playlist_id: String, sender: mpsc::Sender<Out<E>>) -> Youtube<E> where
        E: IntoIndex,
        E: FromImage<E>,
        E: Clone,
        E: std::fmt::Debug,
        E: std::marker::Send,
    {
        let sender = Arc::new(sender);
        let (send, mut recv) = mpsc::channel::<E>(32);
        let state = Arc::new(State {
            api_key,
            playlist_id,
            last_action: Mutex::new(Instant::now() - DELAY),
            items: Mutex::new(vec![]),
        });

        let rt = Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let state_copy = Arc::clone(&state);
        std::thread::spawn(move || {
            rt.block_on(async move {
                let _ = render_youtube_logo(Arc::clone(&sender)).await;
                let _ = pull_playlist_items(Arc::clone(&state_copy)).await;
                while let Some(event) = recv.recv().await {
                    let mut last_action = state_copy.last_action.lock().unwrap();
                    if last_action.elapsed() > DELAY {
                        tokio::spawn(handle_youtube_task(Arc::clone(&state_copy), Arc::clone(&sender), event.clone()));
                        *last_action = Instant::now();
                    } else {
                        println!("Ignoring event: {:?}", event);
                    }
                }
            });
        });

        Youtube {
            state,
            spawn: send,
        }
    }

    pub fn handle(&self, event: E) where
        E: IntoIndex
    {
        match self.spawn.blocking_send(event) {
            Ok(()) => {},
            Err(_) => panic!("The shared runtime has shut down."),
        }
    }
}

pub const COLOR: [u8; 3] = [255, 0, 0];

pub fn get_youtube_logo() -> Image {
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

async fn render_youtube_logo<E>(sender: Arc<mpsc::Sender<Out<E>>>) -> Result<(), ()> where
    E: FromImage<E>,
    E: std::fmt::Debug,
{
    let event = E::from_image(get_youtube_logo()).map_err(|err| {
        eprintln!("Could not convert the image into a MIDI event: {:?}", err);
        ()
    })?;

    return sender.send(Out::Event(event)).await.map_err(|err| {
        eprintln!("Could not send the event back to the router: {:?}", err);
        ()
    });
}

async fn pull_playlist_items(state: Arc<State>) -> Result<(), client::Error> {
    println!("Pulling Youtube playlist items…");
    let new_items = client::playlist::get_all_items(state.api_key.clone(), state.playlist_id.clone()).await?;
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
            let item = {
                let items = state.items.lock().unwrap();
                items.get(usize::from(index)).map(|item| item.clone())
            };

            match item {
                Some(item) => {
                    let video_id = item.snippet.resource_id.video_id;
                    match sender.send(Out::Command(Command::Play(video_id.clone()))).await {
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
