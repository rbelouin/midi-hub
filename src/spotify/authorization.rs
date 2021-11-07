extern crate tiny_http;
extern crate url;
extern crate querystring;
extern crate serde;
extern crate open;

use std::io::{Error, ErrorKind};

use base64::encode;
use reqwest::header::HeaderMap;
use tiny_http::{Server, Response};
use url::Url;
use serde::Deserialize;

#[derive(Clone, Debug)]
pub struct SpotifyAppConfig {
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

pub async fn authorize(config: &SpotifyAppConfig) -> Result<SpotifyTokenResponse, ()> {
    let _ = spawn_authorization_browser(config).await;
    let token_response = spawn_authorization_server(config).await;

    return match token_response {
        Ok(token_response) => Ok(token_response),
        Err(_) => Err(()),
    };
}

pub async fn spawn_authorization_browser(config: &SpotifyAppConfig) -> Result<(), Box<dyn std::error::Error>> {
    println!("Spawning a browser!");
    let client_id = config.client_id.clone();
    let _ = tokio::task::spawn_blocking(move || {
        return open::that(format!("https://accounts.spotify.com/authorize?client_id={}&response_type=code&scope=user-modify-playback-state&redirect_uri=http://localhost:12345/callback", client_id));
    }).await?;
    return Ok(());
}

pub async fn spawn_authorization_server(config: &SpotifyAppConfig) -> Result<SpotifyTokenResponse, Box<dyn std::error::Error>> {
    println!("Spawning a server!");
    let server = Server::http("0.0.0.0:12345").unwrap();
    let base_url = Url::parse("http://localhost:12345")?;

    for request in server.incoming_requests().take(1) {
        let url = base_url.join(request.url()).unwrap();
        let params = url.query().map(querystring::querify);
        let code = params.unwrap_or(vec![]).iter().find(|(key, _value)| key.clone() == "code").map(|(_key, value)| value.clone());
        match code {
            Some(code) => {
                let response = Response::from_string("You can now close this tab.");
                request.respond(response)?;
                return request_token(config, &String::from(code)).await;
            },
            _ => {
                let response = Response::from_string("An error occurred (see the logs), you may need to go through the authorization flow again.");
                request.respond(response)?;
                return Err(Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, "Code not found in the token response")));
            },
        }
    }

    return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Interrupted, "Server shut down before receiving a request")));
}

pub async fn request_token(config: &SpotifyAppConfig, code: &String) -> Result<SpotifyTokenResponse, Box<dyn std::error::Error>> {
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

pub async fn refresh_token(config: &SpotifyAppConfig) -> Result<SpotifyTokenResponse, Box<dyn std::error::Error>> {
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

fn prepare_headers(config: &SpotifyAppConfig) -> HeaderMap {
    let base64_authorization = encode(format!("{}:{}", config.client_id, config.client_secret));
    let mut headers = HeaderMap::new();
    headers.insert("Authorization", format!("Basic {}", base64_authorization).parse().unwrap());
    headers.insert("Content-Type", "application/x-www-form-urlencoded".parse().unwrap());
    return headers;
}
