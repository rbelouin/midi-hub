use std::collections::HashMap;
use std::time::Duration;

use dialoguer::{theme::ColorfulTheme, Input, Select};
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
    let client_id: String = Input::<String>::with_theme(&ColorfulTheme::default())
        .with_prompt("[spotify] please enter your app client_id:")
        .interact()?
        .trim()
        .to_string();

    let client_secret: String = Input::<String>::with_theme(&ColorfulTheme::default())
        .with_prompt("[spotify] please enter your app client_secret:")
        .interact()?
        .trim()
        .to_string();

    println!("[spotify] using the client credentials to authorize the user...");
    let token = authorize_blocking(&client_id, &client_secret)?;
    let refresh_token = token.refresh_token.clone()
        .expect("[spotify] the authorization flow should have exposed a refresh token");
    println!("");

    println!("[spotify] retrieving available playlists...");
    let playlists = get_playlists_blocking(&token)?;
    let items = playlists.items.iter().map(|item| {
        return format!("{} ({} tracks)", item.name, item.tracks.total);
    }).collect::<Vec<String>>();

    if items.is_empty() {
        panic!("[spotify] no playlists could be found!");
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("[spotify] please select the playlist you want to play via midi-hub:")
        .items(items.as_slice())
        .interact()?;

    let playlist_id = playlists.items[selection].id.clone();

    return Ok(Config {
        playlist_id,
        client_id,
        client_secret,
        refresh_token,
    });
}

fn get_playlists_blocking(token: &SpotifyTokenResponse) -> Result<SpotifyPlaylists, Box<dyn std::error::Error>> {
    let runtime = Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();

    let access_token = token.access_token.clone();
    let result = runtime.block_on(runtime.spawn(async move {
        let client = SpotifyApiClientImpl::new();
        return client.get_playlists(access_token).await
            .map_err(|err| {
                eprintln!("[spotify] could not retrieve user playlists: {}", err);
                return Box::new(err);
            });
    })).map_err(|err| {
        eprintln!("[spotify] could not wait for the asynchronous authorization process to complete: {}", err);
        return Box::new(std::io::Error::from(err));
    });

    return match result {
        Ok(Ok(playlists)) => Ok(playlists),
        Ok(Err(err)) => Err(err),
        Err(err) => Err(err),
    };
}
fn authorize_blocking(client_id: &String, client_secret: &String) -> Result<SpotifyTokenResponse, Box<dyn std::error::Error>> {
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

async fn authorize(client_id: &String, client_secret: &String) -> Result<SpotifyTokenResponse, Box<dyn std::error::Error>> {
    spawn_authorization_browser(client_id).await?;
    return spawn_authorization_server(client_id, client_secret).await;
}

async fn spawn_authorization_browser(client_id: &String) -> Result<(), Box<dyn std::error::Error>> {
    println!("[spotify] opening a browser tab...");
    tokio::time::sleep(Duration::from_millis(3000)).await;
    let client_id = client_id.clone();
    let result = tokio::task::spawn_blocking(move || {
        return open::that(format!("https://accounts.spotify.com/authorize?client_id={}&response_type=code&scope=streaming+user-read-email+user-modify-playback-state+user-read-private+playlist-read-private&redirect_uri=http://localhost:12345/callback", client_id)).map_err(|err| {
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

async fn spawn_authorization_server(client_id: &String, client_secret: &String) -> Result<SpotifyTokenResponse, Box<dyn std::error::Error>> {
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
    let client = SpotifyApiClientImpl::new();
    let token = client.request_token(client_id, client_secret, &code).await?;
    return Ok(token);
}
