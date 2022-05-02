use std::collections::HashMap;
use std::time::Duration;

use serde::{Serialize, Deserialize};
use tokio::runtime::Builder;
use warp::Filter;

use super::client::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub playlist_id: String,
    pub client_id: String,
    pub client_secret: String,
    pub refresh_token: String,
}

pub fn configure() -> Result<Config, Box<dyn std::error::Error>> {
    let mut client_id = String::new();
    let mut client_secret = String::new();
    let mut playlist_id = String::new();

    println!("[spotify] please enter your app client_id: ");
    std::io::stdin().read_line(&mut client_id)?;
    let client_id = client_id.trim().to_string();
    println!("");

    println!("[spotify] please enter your app client_secret: ");
    std::io::stdin().read_line(&mut client_secret)?;
    let client_secret = client_secret.trim().to_string();
    println!("");

    println!("[spotify] using the client credentials to authorize the user...");
    let refresh_token = authorize_blocking(&client_id, &client_secret)?;
    println!("");

    println!("[spotify] please enter the id of the playlist you want to play via midi-hub:");
    std::io::stdin().read_line(&mut playlist_id)?;
    let playlist_id = playlist_id.trim().to_string();
    println!("");

    return Ok(Config {
        playlist_id,
        client_id,
        client_secret,
        refresh_token,
    });
}

fn authorize_blocking(client_id: &String, client_secret: &String) -> Result<String, Box<dyn std::error::Error>> {
    let runtime = Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();

    let client_id = client_id.clone();
    let client_secret = client_secret.clone();
    let result = runtime.block_on(runtime.spawn(async move {
        return authorize(&client_id, &client_secret).await
            .map_err(|err| {
                eprintln!("[spotify] could not authorize the user: {}", err);
                return Box::new(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
            });
    })).map_err(|err| {
        eprintln!("[spotify] could not wait for the asynchronous authorization process to complete: {}", err);
        return Box::new(std::io::Error::from(err));
    });

    return match result {
        Ok(Ok(token)) => Ok(token),
        Ok(Err(err)) => Err(err),
        Err(err) => Err(err),
    };
}

async fn authorize(client_id: &String, client_secret: &String) -> Result<String, Box<dyn std::error::Error>> {
    spawn_authorization_browser(client_id).await?;
    return spawn_authorization_server(client_id, client_secret).await;
}

async fn spawn_authorization_browser(client_id: &String) -> Result<(), Box<dyn std::error::Error>> {
    println!("[spotify] opening a browser tab...");
    tokio::time::sleep(Duration::from_millis(3000)).await;
    let client_id = client_id.clone();
    let result = tokio::task::spawn_blocking(move || {
        return open::that(format!("https://accounts.spotify.com/authorize?client_id={}&response_type=code&scope=streaming+user-read-email+user-modify-playback-state+user-read-private&redirect_uri=http://localhost:12345/callback", client_id)).map_err(|err| {
            eprintln!("[spotify] error when opening the browser tab: {}", err);
            Box::new(std::io::Error::from(err))
        });
    }).await.map_err(|err| {
        eprintln!("[spotify] could not launch a child process: {}", err);
        Box::new(std::io::Error::from(err))
    });

    return match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(err)) => Err(err),
        Err(err) => Err(err),
    };
}

async fn spawn_authorization_server(client_id: &String, client_secret: &String) -> Result<String, Box<dyn std::error::Error>> {
    println!("[spotify] starting a server listening on 0.0.0.0:12345");
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(1usize);
    let (send, recv) = tokio::sync::oneshot::channel::<String>();
    let routes = warp::any()
        .and(warp::query::<HashMap<String, String>>())
        .map(move |query: HashMap<String, String>| {
            let code = query.get("code");
            match code {
                Some(code) => {
                    let _ = tx.try_send(code.to_string());
                    return "You can now close this tab.";
                },
                _ => {
                    let _ = tx.try_send("".to_string());
                    return "An error occurred (see the logs), you may need to go through the authorization flow again.";
                },
            }
        });

    let (_addr, server) = warp::serve(routes)
        .bind_with_graceful_shutdown(([0, 0, 0, 0], 12345), async move {
            let code = rx.recv().await.unwrap_or("".to_string());
            send.send(code).ok();
        });

    server.await;
    let code = recv.await.map_err(|err| Box::new(err))?;
    let token = SPOTIFY_API_CLIENT.request_token(client_id, client_secret, &code).await?;
    return token.refresh_token.ok_or(Box::new(std::io::Error::from(std::io::ErrorKind::InvalidData)));
}
