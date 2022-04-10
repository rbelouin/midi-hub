extern crate portmidi as pm;
extern crate signal_hook as sh;

use std::env;

mod spotify;
mod image;
mod midi;
mod router;
mod youtube;

enum Config {
    LoginConfig {
        config: spotify::SpotifyAppConfig,
    },
    RunConfig {
        config: router::RunConfig,
    },
}

fn main() {
    let result = args().and_then(|config| {
        match config {
            Config::LoginConfig { config } => {
                return spotify::login_sync(config.clone()).and_then(|token| token.refresh_token.ok_or(()))
                    .map(|refresh_token| {
                        println!("Please use this refresh token to start the service: {:?}", refresh_token);
                        return ();
                    })
                    .map_err(|()| String::from("Could not log in"));
            },
            Config::RunConfig { config } => {
                let mut router = router::Router::new(config);
                router.run().map_err(|err| format!("{}", err))
            }
        }
    });

    match result {
        Ok(_) => println!("Completed successfully. Bye!"),
        Err(err) => println!("{}", err),
    }
}

fn args() -> Result<Config, String> {
    let args: Vec<String> = env::args().collect();
    return match args.get(1).map(|s| s.as_str()) {
        Some("login") => {
            return match &args[2..] {
                [client_id, client_secret] => Ok(Config::LoginConfig {
                    config: spotify::SpotifyAppConfig {
                        authorization: spotify::authorization::SpotifyAuthorizationConfig {
                            client_id: String::from(client_id),
                            client_secret: String::from(client_secret),
                            refresh_token: None,
                        },
                        // this is not needed to generate a refresh token
                        playlist_id: "".to_string(),
                    },
                }),
                _ => Err(String::from("Usage: ./midi-hub login <client-id> <client-secret>")),
            };
        },
        Some("run") => {
            return match &args[2..] {
                [client_id, client_secret, input_name, output_name, spotify_selector, playlist_id, token, youtube_device, youtube_api_key, youtube_playlist_id] => Ok(Config::RunConfig {
                    config: router::RunConfig {
                        spotify_app_config: spotify::SpotifyAppConfig {
                            authorization: spotify::authorization::SpotifyAuthorizationConfig {
                                client_id: String::from(client_id),
                                client_secret: String::from(client_secret),
                                refresh_token: Some(String::from(token)),
                            },
                            playlist_id: String::from(playlist_id),
                        },
                        input_name: String::from(input_name),
                        output_name: String::from(output_name),
                        spotify_selector: String::from(spotify_selector),
                        youtube_device: String::from(youtube_device),
                        youtube_api_key: String::from(youtube_api_key),
                        youtube_playlist_id: String::from(youtube_playlist_id),
                    },
                }),
                _ => Err(String::from("Usage: ./midi-hub run <client-id> <client-secret> <input-name> <output-name> <spotify-selector> <playlist-id> <spotify-token> <youtube-device> <youtube-api-key> <youtube-playlist-id>")),
            };
        },
        _ => Err(String::from("Usage ./midi-hub [login|run] <args>")),
    };
}
