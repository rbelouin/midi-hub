extern crate url;
extern crate querystring;
extern crate serde;
extern crate open;
extern crate warp;
extern crate tokio;

use std::collections::HashMap;
use std::io::{Error, ErrorKind};

use base64::encode;
use reqwest::header::HeaderMap;
use serde::Deserialize;
use warp::Filter;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

#[derive(Clone, Debug)]
pub struct SpotifyAuthorizationConfig {
    pub client_id: String,
    pub client_secret: String,
    pub refresh_token: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct SpotifyTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub scope: Option<String>,
    pub expires_in: i16,
    pub refresh_token: Option<String>,
}

pub async fn authorize(config: &SpotifyAuthorizationConfig) -> Result<SpotifyTokenResponse, ()> {
    let _ = spawn_authorization_browser(config).await;
    let token_response = spawn_authorization_server(config).await;

    return match token_response {
        Ok(token_response) => Ok(token_response),
        Err(_) => Err(()),
    };
}

pub async fn spawn_authorization_browser(config: &SpotifyAuthorizationConfig) -> Result<(), Box<dyn std::error::Error>> {
    println!("Spawning a browser!");
    let client_id = config.client_id.clone();
    let _ = tokio::task::spawn_blocking(move || {
        return open::that(format!("https://accounts.spotify.com/authorize?client_id={}&response_type=code&scope=user-modify-playback-state%20user-read-playback-state&redirect_uri=http://localhost:12345/callback", client_id));
    }).await?;
    return Ok(());
}

pub async fn spawn_authorization_server(config: &SpotifyAuthorizationConfig) -> Result<SpotifyTokenResponse, Box<dyn std::error::Error>> {
    println!("Spawning a server!");
    let (tx, mut rx) = mpsc::channel::<String>(1usize);
    let (send, recv) = oneshot::channel::<String>();
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
    return request_token(config, &code).await;
}

pub async fn request_token(config: &SpotifyAuthorizationConfig, code: &String) -> Result<SpotifyTokenResponse, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let response = client.post("https://accounts.spotify.com/api/token")
        .headers(prepare_headers(config))
        .body(querystring::stringify(vec![
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", "http://localhost:12345/callback"),
        ]))
        .send()
        .await?;

    return Ok(response
        .json::<SpotifyTokenResponse>()
        .await?);
}

pub async fn refresh_token(config: &SpotifyAuthorizationConfig) -> Result<SpotifyTokenResponse, Box<dyn std::error::Error>> {
    match &config.refresh_token {
        Some(token) => {
            let client = reqwest::Client::new();
            let response = client.post("https://accounts.spotify.com/api/token")
                .headers(prepare_headers(config))
                .body(querystring::stringify(vec![
                    ("grant_type", "refresh_token"),
                    ("refresh_token", token),
                ]))
                .send()
                .await?;

            return Ok(response
                .json::<SpotifyTokenResponse>()
                .await?);
        },
        None => {
            println!("Error: please log in so that you get a refresh token");
            return Err(Box::new(Error::from(ErrorKind::NotFound)));
        },
    }
}

fn prepare_headers(config: &SpotifyAuthorizationConfig) -> HeaderMap {
    let base64_authorization = encode(format!("{}:{}", config.client_id, config.client_secret));
    let mut headers = HeaderMap::new();
    headers.insert("Authorization", format!("Basic {}", base64_authorization).parse().unwrap());
    headers.insert("Content-Type", "application/x-www-form-urlencoded".parse().unwrap());
    return headers;
}
