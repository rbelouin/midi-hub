use tokio::runtime::Builder;
use tokio::sync::mpsc;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::midi::IntoIndex;
use super::Command;

#[derive(Clone)]
pub struct Youtube<E> {
    state: Arc<State>,
    spawn: mpsc::Sender<E>,
}

struct State {
    last_action: Mutex<Instant>,
    items: Vec<String>,
}

const DELAY: Duration = Duration::from_millis(5_000);

impl<E: 'static> Youtube<E> {
    pub fn new(sender: mpsc::Sender<Command>) -> Youtube<E> where
        E: IntoIndex,
        E: Clone,
        E: std::fmt::Debug,
        E: std::marker::Send,
    {
        let sender = Arc::new(sender);
        let (send, mut recv) = mpsc::channel::<E>(32);
        let state = Arc::new(State {
            last_action: Mutex::new(Instant::now() - DELAY),
            items: vec![
                "Dy-WpCFz1j4".to_string(),
                "CIEQJStNdko".to_string(),
            ],
        });

        let rt = Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let state_copy = Arc::clone(&state);
        std::thread::spawn(move || {
            rt.block_on(async move {
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

async fn handle_youtube_task<E>(state: Arc<State>, sender: Arc<mpsc::Sender<Command>>, event_in: E) where
    E: IntoIndex,
    E: std::fmt::Debug,
{
    match event_in.into_index() {
        Ok(Some(index)) => {
            let item = state.items.get(usize::from(index)).map(|item| item.clone());

            match item {
                Some(video_id) => {
                    match sender.send(Command::Play(video_id.clone())).await {
                        Ok(_) => println!("Playing track {}", video_id),
                        Err(_) => eprintln!("Could not play track {}", video_id),
                    }
                },
                _ => println!("No track for index: {}", index),
            }
        },
        _ => {},
    };
}
