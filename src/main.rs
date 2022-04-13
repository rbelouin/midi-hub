extern crate portmidi as pm;
extern crate signal_hook as sh;

use std::env;

mod spotify;
mod image;
mod midi;
mod router;
mod youtube;
mod server;

enum Config {
    LoginConfig {
        config: spotify::authorization::Config,
    },
    RunConfig {
        config: router::RunConfig,
    },
}

fn main() {
    let result = args().and_then(|config| {
        match config {
            Config::LoginConfig { config } => {
                return spotify::authorization::login_sync(config.clone()).and_then(|token| token.refresh_token.ok_or(()))
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
                    config: spotify::authorization::Config {
                        client_id: String::from(client_id),
                        client_secret: String::from(client_secret),
                        refresh_token: None,
                    },
                }),
                _ => Err(String::from("Usage: ./midi-hub login <client-id> <client-secret>")),
            };
        },
        Some("run") => {
            return match &args[2..] {
                [client_id, client_secret, input_name, output_name, launchpad_name, playlist_id, token, youtube_api_key, youtube_playlist_id] => Ok(Config::RunConfig {
                    config: router::RunConfig {
                        input_name: String::from(input_name),
                        output_name: String::from(output_name),
                        launchpad_name: String::from(launchpad_name),
                        spotify_config: spotify::Config {
                            authorization: spotify::authorization::Config {
                                client_id: String::from(client_id),
                                client_secret: String::from(client_secret),
                                refresh_token: Some(String::from(token)),
                            },
                            playlist_id: String::from(playlist_id),
                        },
                        youtube_config: youtube::Config {
                            api_key: String::from(youtube_api_key),
                            playlist_id: String::from(youtube_playlist_id),
                        }
                    },
                }),
                _ => Err(String::from("Usage: ./midi-hub run <client-id> <client-secret> <input-name> <output-name> <launchpad-name> <playlist-id> <spotify-token> <youtube-api-key> <youtube-playlist-id>")),
            };
        },
        _ => Err(String::from("Usage ./midi-hub [login|run] <args>")),
    };
}
