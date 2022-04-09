pub use reqwest::{Client, Error};
use serde::{Serialize, Deserialize};

pub mod playlist {
    use super::*;

    #[derive(Clone, Debug, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Playlist {
        pub items: Vec<PlaylistItem>,
        pub next_page_token: Option<String>,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct PlaylistItem {
        pub snippet: PlaylistItemSnippet,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct PlaylistItemSnippet {
        pub title: String,
        pub resource_id: PlaylistItemSnippetResourceId,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct PlaylistItemSnippetResourceId {
        pub video_id: String,
    }

    pub async fn get_paginated_items(
        api_key: &String,
        playlist_id: &String,
        max_results: u8,
        page_token: &Option<String>,
    ) -> Result<Playlist, Error> {
        let page_token = page_token
            .as_ref()
            .map(|token| format!("&pageToken={}", token))
            .unwrap_or("".to_string());

        let client = Client::new();
        let response = client.get(
            format!("https://youtube.googleapis.com/youtube/v3/playlistItems?part=snippet&maxResults={}&playlistId={}&key={}{}", max_results, playlist_id, api_key, page_token))
            .send()
            .await?;

        let playlist = response
            .json::<Playlist>()
            .await?;

        return Ok(playlist);
    }

    pub async fn get_all_items(
        api_key: String,
        playlist_id: String,
    ) -> Result<Vec<PlaylistItem>, Error> {
        let mut page_token = None;
        let mut all_items = vec![];

        loop {
            let playlist = get_paginated_items(&api_key, &playlist_id, 50, &page_token).await;
            match playlist {
                Err(err) => {
                    return Err(err);
                },
                Ok(mut playlist) => {
                    page_token = playlist.next_page_token;
                    all_items.append(&mut playlist.items);

                    if page_token.is_none() {
                        return Ok(all_items);
                    }
                },
            }
        }
    }
}

#[cfg(test)]
mod test {
    #[test]
    pub fn test_get_paginated_items() {
        use tokio::runtime::Builder;

        let api_key = std::env::var("YOUTUBE_API_KEY").expect("YOUTUBE_API_KEY must be defined");
        let playlist_id = "PLI6XHoityAbgl2aYvt_RckR4-P4G_HPsv".to_string();

        Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async move {
                let playlist = super::playlist::get_paginated_items(&api_key, &playlist_id, 32, &None).await
                    .expect("retrieving playlist items should not fail");

                assert_eq!(playlist.items.len(), 32);

                let title = playlist.items.into_iter().find(|item| {
                    return item.snippet.resource_id.video_id == "Dy-WpCFz1j4";
                }).map(|item| item.snippet.title);

                assert_eq!(title, Some("Kompisbandet - Krokodilen i bilen".to_string()));
            });
    }

    #[test]
    pub fn test_get_all_items() {
        use tokio::runtime::Builder;

        let api_key = std::env::var("YOUTUBE_API_KEY").expect("YOUTUBE_API_KEY must be defined");
        let playlist_id = "PLI6XHoityAbgl2aYvt_RckR4-P4G_HPsv".to_string();

        Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async move {
                let items = super::playlist::get_all_items(api_key, playlist_id).await
                    .expect("retrieving playlist items should not fail");

                assert_eq!(items.len(), 64);

                let title = items.into_iter().find(|item| {
                    return item.snippet.resource_id.video_id == "Dy-WpCFz1j4";
                }).map(|item| item.snippet.title);

                assert_eq!(title, Some("Kompisbandet - Krokodilen i bilen".to_string()));
            });
    }
}
