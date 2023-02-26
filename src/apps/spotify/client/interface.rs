use serde::Deserialize;

#[cfg(test)]
use mockall::automock;

pub type SpotifyApiResult<A> = std::result::Result<A, SpotifyApiError>;

#[cfg_attr(test, automock)]
#[async_trait]
pub trait SpotifyApiClient {
    async fn request_token(
        &self,
        client_id: &String,
        client_secret: &String,
        code: &String,
    ) -> SpotifyApiResult<SpotifyTokenResponse>;

    async fn refresh_token(
        &self,
        client_id: &String,
        client_secret: &String,
        refresh_token: &String,
    ) -> SpotifyApiResult<SpotifyTokenResponse>;

    async fn get_playlists(
        &self,
        token: String,
    ) -> SpotifyApiResult<SpotifyPlaylists>;

    async fn get_playlist_tracks(
        &self,
        token: String,
        playlist_id: String
    ) -> SpotifyApiResult<Vec<SpotifyTrack>>;

    async fn get_playback_state(
        &self,
        token: String
    ) -> SpotifyApiResult<Option<SpotifyPlaybackState>>;

    async fn start_or_resume_playback(
        &self,
        token: String,
        uris: Vec<String>,
        device_id: Option<String>,
    ) -> SpotifyApiResult<()>;

    async fn pause_playback(
        &self,
        token: String,
    ) -> SpotifyApiResult<()>;

    async fn get_available_devices(
        &self,
        token: String
    ) -> SpotifyApiResult<SpotifyDevices>;
}

#[derive(Debug)]
pub enum SpotifyApiError {
    Unauthorized,
    Other(Box<dyn std::error::Error + Send>),
}

impl std::fmt::Display for SpotifyApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            SpotifyApiError::Unauthorized => {
                write!(f, "Unauthorized access to Spotify Web API")
            },
            SpotifyApiError::Other(err) => std::fmt::Display::fmt(err, f),
        }
    }
}

impl std::error::Error for SpotifyApiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self {
            SpotifyApiError::Unauthorized => None,
            SpotifyApiError::Other(err) => err.source(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SpotifyTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub scope: Option<String>,
    pub expires_in: i16,
    pub refresh_token: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SpotifyAlbumImage {
    pub width: u16,
    pub height: u16,
    pub url: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SpotifyAlbum {
    pub images: Vec<SpotifyAlbumImage>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SpotifyTrack {
    pub id: String,
    pub name: String,
    pub uri: String,
    pub album: SpotifyAlbum,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SpotifyPlaylistResponse {
    pub href: String,
    pub items: Vec<SpotifyPlaylistItem>
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub  struct SpotifyPlaylistItem {
    pub track: SpotifyTrack,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SpotifyPlaybackState {
    pub is_playing: bool,
    pub item: SpotifyTrack,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SpotifyDevices {
    pub devices: Vec<SpotifyDevice>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SpotifyDevice {
    pub id: String,
    pub is_active: bool,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SpotifyPlaylists {
    pub items: Vec<SpotifyPlaylist>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SpotifyPlaylist {
    pub id: String,
    pub name: String,
    pub tracks: SpotifyPlaylistTracks,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SpotifyPlaylistTracks {
    pub total: u16,
}
