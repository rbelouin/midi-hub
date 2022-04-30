extern crate futures_util;

use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use serde::{Serialize, Deserialize};
use tokio::sync::RwLock;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Sender, Receiver};
use tokio::sync::mpsc::error::TryRecvError;
use tokio::runtime::Builder;
use warp::Filter;
use warp::ws::{Message, WebSocket, Ws};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    SpotifyPlay { track_id: String, access_token: String },
    SpotifyPause,
    YoutubePlay { video_id: String },
    YoutubePause,
}

pub struct HttpServer {
    sender: Arc<RwLock<Sender<Command>>>,
    receiver: Arc<Mutex<Receiver<Command>>>,
}

impl HttpServer {
    pub fn start() -> Self {
        let (tx, rx) = mpsc::channel::<Command>(1usize);
        let sender = Arc::new(RwLock::new(tx));
        let receiver = Arc::new(Mutex::new(rx));

        let thread_sender = Arc::clone(&sender);
        let thread_receiver = Arc::clone(&receiver);
        std::thread::spawn(move || {
            Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move {
                    let public = warp::any()
                        .and(warp::fs::dir("public"));

                    let websocket_sender = Arc::clone(&thread_sender);
                    let websocket_receiver = Arc::clone(&thread_receiver);
                    let websocket = warp::path("ws")
                        .and(warp::ws())
                        .map(move |ws: Ws| {
                            let websocket_sender = Arc::clone(&websocket_sender);
                            let websocket_receiver = Arc::clone(&websocket_receiver);
                            ws.on_upgrade(move |ws| handle_connection(ws, Arc::clone(&websocket_sender), Arc::clone(&websocket_receiver)))
                        });

                    let routes = public
                        .or(websocket);

                    println!("HTTP server listening on http://localhost:54321/");
                    warp::serve(routes)
                        .run(([0, 0, 0, 0], 54321))
                        .await;
                });
        });

        HttpServer {
            sender,
            receiver,
        }
    }

    pub fn send(&self, command: Command) {
        self.sender.try_read().expect("sender should be readable").blocking_send(command)
            .unwrap_or_else(|err| eprintln!("Error: {:?}", err));
    }

    pub fn receive(&self) -> Result<Command, TryRecvError> {
        let mut receiver = self.receiver.lock().expect("receiver should be available");
        receiver.try_recv()
    }
}

async fn handle_connection(ws: WebSocket, sender: Arc<RwLock<Sender<Command>>>, receiver: Arc<Mutex<Receiver<Command>>>) {
    let (sender_tx, mut sender_rx) = mpsc::channel::<Command>(1usize);
    let (receiver_tx, receiver_rx) = mpsc::channel::<Command>(1usize);
    let (mut ws_tx, mut ws_rx) = ws.split();

    let mut sender = sender.write().await;
    *sender = sender_tx;

    let mut receiver = receiver.lock().expect("receiver should be available");
    *receiver = receiver_rx;

    tokio::task::spawn(async move {
        while let Some(command) = ws_rx.next().await {
            match command.as_ref().map_err(|_| ()).and_then(|c| c.to_str()) {
                Ok(command) => {
                    match serde_json::from_str::<Command>(command) {
                        Ok(command) => {
                            println!("[server] received command {:?}", command);
                            receiver_tx.send(command).await.unwrap_or_else(|err| {
                                eprintln!("[server] could not forward the received command back to the router: {}", err);
                            });
                        },
                        Err(err) => eprintln!("[server] could not parse the command: {}", err),
                    }
                },
                _ => eprintln!("[server] error when receiving command: {:?}", command),
            }
        }
    });

    tokio::task::spawn(async move {
        while let Some(command) = sender_rx.recv().await {
            println!("Sending {:?}", command);
            let _ = ws_tx.send(Message::text(serde_json::to_string(&command).unwrap_or("Error when serializing command".to_string()))).await;
        }
    });
}
