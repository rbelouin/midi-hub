use tokio::runtime::Builder;
use tokio::sync::mpsc;

use std::convert::Into;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::apps::{App, Out, ServerCommand};
use crate::image::Image;
use crate::midi::{Event, EventTransformer};

use super::config::Config;
use super::client;

struct State {
    input_transformer: &'static (dyn EventTransformer + Sync),
    output_transformer: &'static (dyn EventTransformer + Sync),
    config: Config,
    last_action: Mutex<Instant>,
    items: Mutex<Vec<client::playlist::PlaylistItem>>,
    playing: Mutex<Option<u16>>,
}

pub struct Youtube {
    in_sender: mpsc::Sender<Event>,
    out_receiver: mpsc::Receiver<Out>,
}

pub const NAME: &'static str = "youtube";
pub const COLOR: [u8; 3] = [255, 0, 0];

const DELAY: Duration = Duration::from_millis(5_000);

impl Youtube {
    pub fn new(
        config: Config,
        input_transformer: &'static (dyn EventTransformer + Sync),
        output_transformer: &'static (dyn EventTransformer + Sync),
    ) -> Self {
        let (in_sender, mut in_receiver) = mpsc::channel::<Event>(32);
        let (out_sender, out_receiver) = mpsc::channel::<Out>(32);

        let state = Arc::new(State {
            input_transformer,
            output_transformer,
            config,
            last_action: Mutex::new(Instant::now() - DELAY),
            items: Mutex::new(vec![]),
            playing: Mutex::new(None),
        });

        let rt = Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let state_copy = Arc::clone(&state);
        let out_sender = Arc::new(out_sender);
        std::thread::spawn(move || {
            rt.block_on(async move {
                let _ = render_youtube_logo(Arc::clone(&state_copy), Arc::clone(&out_sender)).await;
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

impl App for Youtube {
    fn get_name(&self) -> &'static str {
        return NAME;
    }

    fn get_color(&self) -> [u8; 3] {
        return COLOR;
    }

    fn get_logo(&self) -> Image {
        return get_logo();
    }

    fn send(&mut self, event: Event) -> Result<(), mpsc::error::SendError<Event>> {
        return self.in_sender.blocking_send(event);
    }

    fn receive(&mut self) -> Result<Out, mpsc::error::TryRecvError> {
        return self.out_receiver.try_recv();
    }
}

async fn render_youtube_logo(state: Arc<State>, sender: Arc<mpsc::Sender<Out>>) -> Result<(), ()> {
    let event = state.output_transformer.from_image(get_logo()).map_err(|err| {
        eprintln!("Could not convert the image into a MIDI event: {:?}", err);
        ()
    })?;

    sender.send(event.into()).await.unwrap_or_else(|err| {
        eprintln!("Could not send the event back to the router: {:?}", err);
    });

    let playing_index = {
        let playing = state.playing.lock().expect("we should be able to lock state.playing");
        playing.clone()
    };

    if let Some(index) = playing_index {
        let event = state.output_transformer.from_index_to_highlight(index).map_err(|err| {
            eprintln!("Could not convert the index to highlight into a  MIDI event: {:?}", err)
        })?;
        sender.send(event.into()).await.unwrap_or_else(|err| {
            eprintln!("Could not send the event back to the router: {:?}", err);
        });
    }

    Ok(())
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

async fn handle_youtube_task(state: Arc<State>, sender: Arc<mpsc::Sender<Out>>, event: Event) {
    match state.input_transformer.into_index(event) {
        Ok(Some(index)) => {
            let playing_index = {
                let playing = state.playing.lock().expect("we should be able to lock state.playing");
                playing.clone()
            };

            if playing_index == Some(index) {
                sender.send(ServerCommand::YoutubePause.into()).await.unwrap_or_else(|err| {
                    eprintln!("[youtube] could not send pause command: {}", err);
                });

                {
                    let mut playing = state.playing.lock().expect("we should be able to lock state.playing");
                    *playing = None;
                }

                render_youtube_logo(state, sender).await.unwrap_or_else(|err| {
                    eprintln!("[youtube] could not render logo: {:?}", err);
                });

                return;
            }

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
                        Ok(_) => {
                            println!("Playing track {}", video_id);
                            {
                                let mut playing = state.playing.lock().expect("we should be able to lock state.playing");
                                *playing = Some(index);
                            }
                            render_youtube_logo(Arc::clone(&state), sender).await.unwrap_or_else(|err| {
                                eprintln!("[youtube] could not render logo: {:?}", err);
                            });
                        },
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
