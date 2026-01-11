use anyhow::Result;
use async_trait::async_trait;

/// Type of search result
#[derive(Debug, Clone, PartialEq)]
pub enum SearchResultType {
    Artist,
    Album,
    Song,
    Playlist,
    Genre,
}

/// A search result from play_any()
#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    /// Display text (e.g., "Artist: Beethoven")
    pub display: String,
    /// The actual value to use for playback
    pub value: String,
    /// Type of result
    pub result_type: SearchResultType,
    /// Match score (0.0 - 1.0)
    pub score: f64,
}

#[async_trait]
pub trait MediaPlayer: Send + Sync {
    /// Play tracks of a specific genre
    async fn play_genre(&self, genre: &str) -> Result<()>;

    /// Play random tracks from the library
    async fn play_random(&self) -> Result<()>;

    /// Play tracks by a specific artist
    async fn play_artist(&self, artist: &str) -> Result<()>;

    /// Play a specific album
    async fn play_album(&self, album: &str) -> Result<()>;

    /// Play a specific song/track
    async fn play_song(&self, song: &str) -> Result<()>;

    /// Play a specific playlist
    async fn play_playlist(&self, playlist: &str, shuffle: bool) -> Result<()>;

    /// Universal search - returns candidates for disambiguation
    /// Matches Python's play_any() which returns (status, message, selection_list)
    async fn play_any(&self, query: &str) -> Result<Vec<SearchResult>> {
        // Default: no universal search support
        let _ = query;
        Ok(vec![])
    }

    /// Get all artists in the library (for Ollama context)
    async fn get_all_artists(&self, limit: usize) -> Vec<String> {
        let _ = limit;
        vec![]
    }

    /// Get albums by a specific artist
    async fn get_artist_albums(&self, artist: &str) -> Vec<String> {
        let _ = artist;
        vec![]
    }

    /// List tracks for current album/context
    async fn list_tracks(&self) -> Vec<(String, String)> {
        // Returns (track_title, track_path/key)
        vec![]
    }

    /// Toggle play/pause
    async fn play_pause(&self) -> Result<()>;

    /// Skip to the next track
    async fn next_track(&self) -> Result<()>;

    /// Go back to the previous track
    async fn previous_track(&self) -> Result<()>;

    /// Increase volume
    async fn volume_up(&self) -> Result<()>;

    /// Decrease volume
    async fn volume_down(&self) -> Result<()>;

    /// Stop playback
    async fn stop(&self) -> Result<()>;

    /// Return the currently playing track info
    async fn what_is_playing(&self) -> Result<String>;

    /// Check if the player is available
    async fn health_check(&self) -> bool;
}

pub mod elisa;
pub mod jriver;
pub mod mpris;
pub mod mpris_utils;
pub mod strawberry;

/// Get the media player based on configuration
pub fn get_player(
    config: &crate::config::Config,
    library: std::sync::Arc<crate::library::LocalLibrary>,
) -> Box<dyn MediaPlayer> {
    match config.player.to_lowercase().as_str() {
        "strawberry" => Box::new(strawberry::StrawberryPlayer::new(std::path::PathBuf::from(
            &config.strawberry_db_path,
        ))),
        "jriver" => Box::new(jriver::JRiverPlayer::new(config)),
        _ => {
            // Default to MPRIS for generic support
            Box::new(mpris::MprisPlayer::new(
                config.mpris_service.clone(),
                library,
            ))
        }
    }
}
