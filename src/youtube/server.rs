extern crate futures_util;

use std::sync::Arc;
use std::time::Duration;

use futures_util::SinkExt;
use serde::{Serialize, Deserialize};
use tokio::sync::RwLock;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio::runtime::Builder;
use warp::Filter;
use warp::ws::{Message, WebSocket, Ws};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Command {
    Play(String),
}

pub struct HttpServer {}

impl HttpServer {
    pub fn start() -> Self {
        let (tx, _) = mpsc::channel::<Command>(1usize);
        let sender = Arc::new(RwLock::new(tx));

        std::thread::spawn(move || {
            Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move {
                    let public = warp::any()
                        .and(warp::fs::dir("public"));

                    let websocket_sender = Arc::clone(&sender);
                    let websocket = warp::path("ws")
                        .and(warp::ws())
                        .map(move |ws: Ws| {
                            let websocket_sender = Arc::clone(&websocket_sender);
                            ws.on_upgrade(move |ws| handle_connection(ws, Arc::clone(&websocket_sender)))
                        });

                    let routes = public
                        .or(websocket);

                    println!("HTTP server listening on http://localhost:54321/");
                    warp::serve(routes)
                        .run(([0, 0, 0, 0], 54321))
                        .await;
                });
        });

        HttpServer {}
    }
}

async fn handle_connection(ws: WebSocket, sender: Arc<RwLock<Sender<Command>>>) {
    let mut ws = ws;

    let (tx, mut rx) = mpsc::channel::<Command>(1usize);
    let mut sender = sender.write().await;
    *sender = tx;

    tokio::task::spawn(async move {
        while let Some(command) = rx.recv().await {
            println!("Sending {:?}", command);
            let _ = ws.send(Message::text(serde_json::to_string(&command).unwrap_or("Error when serializing command".to_string()))).await;
        }
    });

    tokio::time::sleep(Duration::from_millis(10_000)).await;
    let _ = sender.send(Command::Play("w2sF0Gn4UcQ".to_string())).await;
}
