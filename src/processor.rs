//! Command Processor
//!
//! Handles voice command routing and execution for the voice assistant.
//! Delegates to appropriate handlers based on command type.

use crate::players::{MediaPlayer, SearchResult};
use crate::selection::SelectionHandler;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Simple media control keywords (no Ollama needed)
const SIMPLE_CONTROLS: &[&str] = &[
    "next",
    "previous",
    "pause",
    "stop",
    "resume",
    "volume up",
    "volume down",
    "louder",
    "quieter",
    "what's playing",
    "what is playing",
];

/// Keywords that indicate a media/music command
const MEDIA_KEYWORDS: &[&str] = &[
    "play", "pause", "stop", "next", "previous", "volume", "louder", "quieter", "shuffle", "album",
    "artist", "track", "music", "song", "resume", "skip", "what",
];

/// Result of processing a command
#[derive(Debug)]
pub enum ProcessResult {
    /// Command was handled successfully
    Handled,
    /// Command should trigger a search with results
    Search(Vec<SearchResult>),
    /// Speak this text to user
    Speak(String),
    /// User requested quit
    Quit,
    /// Command not recognized
    NotRecognized,
}

pub struct Processor {
    /// Media player
    player: Arc<Box<dyn MediaPlayer>>,
    /// Game mode enabled
    game_mode: bool,
}

impl Processor {
    pub fn new(player: Arc<Box<dyn MediaPlayer>>) -> Self {
        Self {
            player,
            game_mode: false,
        }
    }

    /// Set game mode status
    pub fn set_game_mode(&mut self, enabled: bool) {
        self.game_mode = enabled;
    }

    /// Process a voice command with smart routing
    ///
    /// **Routing Strategy:**
    /// 1. Simple media controls → Fast keywords (always <100ms)
    /// 2. Complex queries + Gaming → Ollama (prevents misrouting)
    /// 3. Complex queries + Not Gaming → Keyword fallback
    pub async fn process(
        &self,
        text: &str,
        selection_handler: &mut SelectionHandler,
    ) -> ProcessResult {
        let text = self.preprocess_text(text);
        let text_lower = text.to_lowercase();

        debug!("Processing command: '{}'", text_lower);

        // Strategy 1: Simple media controls → Always fast keywords
        if Self::is_simple_control(&text_lower) {
            debug!("Simple media control, using fast keywords");
            if let Some(result) = self.handle_media_control(&text_lower).await {
                return result;
            }
        }

        // Check if this looks like a music command
        let is_music_command = Self::quick_media_check(&text_lower);

        if is_music_command {
            debug!("Music command detected");

            // Try media controls first
            if let Some(result) = self.handle_media_control(&text_lower).await {
                return result;
            }

            // Try playback commands
            if let Some(result) = self
                .handle_playback_command(&text_lower, selection_handler)
                .await
            {
                return result;
            }

            warn!("Music command not matched: {}", text_lower);
            return ProcessResult::NotRecognized;
        }

        // Non-music commands
        debug!("Non-music command, checking system/game commands");

        // Quit commands
        if Self::is_quit_command(&text_lower) {
            return ProcessResult::Quit;
        }

        // Help command
        if text_lower == "list commands" || text_lower == "help" {
            return ProcessResult::Speak(
                "You can say play artist or album, stop, next, quit, or what's playing."
                    .to_string(),
            );
        }

        // Game mode toggle
        if text_lower.contains("enable game mode") {
            return ProcessResult::Speak("Game mode enabled.".to_string());
        }
        if text_lower.contains("disable game mode") {
            return ProcessResult::Speak("Game mode disabled.".to_string());
        }

        ProcessResult::NotRecognized
    }

    /// Fast heuristic to detect music commands (2ms overhead)
    fn quick_media_check(text: &str) -> bool {
        let words: Vec<&str> = text.split_whitespace().take(2).collect();
        MEDIA_KEYWORDS
            .iter()
            .any(|kw| words.iter().any(|w| w.contains(kw)))
    }

    /// Check if this is a simple control command
    fn is_simple_control(text: &str) -> bool {
        SIMPLE_CONTROLS
            .iter()
            .any(|ctrl| text.starts_with(ctrl) || text == *ctrl)
    }

    /// Check for quit commands
    fn is_quit_command(text: &str) -> bool {
        [
            "quit",
            "exit",
            "stop listening",
            "goodbye",
            "good bye",
            "leave",
        ]
        .iter()
        .any(|w| text.contains(w))
    }

    /// Preprocess text (common corrections)
    fn preprocess_text(&self, text: &str) -> String {
        let mut result = text.to_string();

        // Common misrecognition: "but" → "play"
        if result.starts_with("but ") {
            result = result.replacen("but ", "play ", 1);
            debug!("Corrected 'but' to 'play': {}", result);
        }

        result
    }

    /// Handle media control commands (pause, next, stop, etc.)
    async fn handle_media_control(&self, text: &str) -> Option<ProcessResult> {
        if text.contains("pause") {
            let _ = self.player.play_pause().await;
            return Some(ProcessResult::Handled);
        }

        if text.contains("stop") && !text.contains("stop listening") {
            let _ = self.player.stop().await;
            return Some(ProcessResult::Handled);
        }

        if text == "play" || text == "resume" || text == "start music" {
            let _ = self.player.play_pause().await;
            return Some(ProcessResult::Handled);
        }

        if text.contains("next track")
            || text.contains("next song")
            || text == "next"
            || text == "skip"
        {
            let _ = self.player.next_track().await;
            return Some(ProcessResult::Handled);
        }

        if text.contains("previous track")
            || text.contains("previous song")
            || text == "previous"
            || text == "back"
        {
            let _ = self.player.previous_track().await;
            return Some(ProcessResult::Handled);
        }

        if text.contains("volume up") || text.contains("louder") {
            let _ = self.player.volume_up().await;
            return Some(ProcessResult::Handled);
        }

        if text.contains("volume down") || text.contains("quieter") {
            let _ = self.player.volume_down().await;
            return Some(ProcessResult::Handled);
        }

        if text.contains("what is playing")
            || text.contains("what's playing")
            || text.contains("what song")
            || text.contains("what track")
        {
            match self.player.what_is_playing().await {
                Ok(info) => return Some(ProcessResult::Speak(info)),
                Err(_) => {
                    return Some(ProcessResult::Speak(
                        "Unable to determine what's playing".to_string(),
                    ))
                }
            }
        }

        None
    }

    /// Handle complex playback commands (searches, playlists, etc.)
    async fn handle_playback_command(
        &self,
        text: &str,
        selection_handler: &mut SelectionHandler,
    ) -> Option<ProcessResult> {
        // Play playlist
        if text.starts_with("play playlist ") || text.starts_with("play smartlist ") {
            let query = text
                .replace("play playlist ", "")
                .replace("play smartlist ", "");
            let shuffle = text.contains("random") || text.contains("shuffle");

            if !query.is_empty() {
                let _ = self.player.play_playlist(&query, shuffle).await;
                return Some(ProcessResult::Speak(format!("Playing playlist {}", query)));
            }
        }

        // Play artist
        if text.starts_with("play artist ") {
            let query = text.replace("play artist ", "");
            if !query.is_empty() {
                let _ = self.player.play_artist(&query).await;
                return Some(ProcessResult::Speak(format!("Playing {}", query)));
            }
        }

        // Play album
        if text.starts_with("play album ") {
            let query = text.replace("play album ", "");
            if !query.is_empty() {
                let _ = self.player.play_album(&query).await;
                return Some(ProcessResult::Speak(format!("Playing {}", query)));
            }
        }

        // Search command
        if text.starts_with("search for ") || text.starts_with("find ") {
            let query = text.replace("search for ", "").replace("find ", "");
            if !query.is_empty() {
                match self.player.play_any(&query).await {
                    Ok(results) if !results.is_empty() => {
                        selection_handler.set_items(results.clone(), "search");
                        return Some(ProcessResult::Search(results));
                    }
                    _ => {
                        return Some(ProcessResult::Speak(format!(
                            "No results found for {}",
                            query
                        )));
                    }
                }
            }
        }

        // Play whatever / random
        if text == "play whatever" || text == "whatever" || text == "play random" {
            let _ = self.player.play_random().await;
            return Some(ProcessResult::Speak("Playing random music".to_string()));
        }

        // Play random genre
        if text.starts_with("play random ") {
            let genre = text.replace("play random ", "");
            if !genre.is_empty() {
                let _ = self.player.play_genre(&genre).await;
                return Some(ProcessResult::Speak(format!(
                    "Playing random {} music",
                    genre
                )));
            }
        }

        // Generic play command → Universal search
        if text.starts_with("play ") {
            let query = text.replace("play ", "");
            if !query.is_empty() {
                match self.player.play_any(&query).await {
                    Ok(results) if !results.is_empty() => {
                        // If only one result with high score, play directly
                        if results.len() == 1
                            || (results[0].score > 0.9
                                && (results.len() == 1
                                    || results[0].score - results[1].score > 0.15))
                        {
                            let best = &results[0];
                            info!(
                                "🎯 Auto-playing best match: {} ({:.2})",
                                best.display, best.score
                            );

                            use crate::players::SearchResultType;
                            match best.result_type {
                                SearchResultType::Artist => {
                                    let _ = self.player.play_artist(&best.value).await;
                                }
                                SearchResultType::Album => {
                                    let _ = self.player.play_album(&best.value).await;
                                }
                                SearchResultType::Playlist => {
                                    let _ = self.player.play_playlist(&best.value, false).await;
                                }
                                SearchResultType::Song => {
                                    let _ = self.player.play_song(&best.value).await;
                                }
                                SearchResultType::Genre => {
                                    let _ = self.player.play_genre(&best.value).await;
                                }
                            }
                            return Some(ProcessResult::Speak(format!("Playing {}", best.value)));
                        }

                        // Multiple results → Selection
                        selection_handler.set_items(results.clone(), "search");
                        return Some(ProcessResult::Search(results));
                    }
                    Ok(_) => {
                        return Some(ProcessResult::Speak(format!(
                            "No results found for {}",
                            query
                        )));
                    }
                    Err(e) => {
                        warn!("Search error: {}", e);
                        return Some(ProcessResult::Speak(format!("Search failed for {}", query)));
                    }
                }
            } else {
                // Just "play" with no query
                let _ = self.player.play_pause().await;
                return Some(ProcessResult::Handled);
            }
        }

        // List albums by artist
        if text.starts_with("list albums by ") || text.starts_with("show albums by ") {
            let artist = text
                .replace("list albums by ", "")
                .replace("show albums by ", "");
            if !artist.is_empty() {
                let albums = self.player.get_artist_albums(&artist).await;
                if !albums.is_empty() {
                    let album_list = albums.join(", ");
                    return Some(ProcessResult::Speak(format!(
                        "Albums by {}: {}",
                        artist, album_list
                    )));
                } else {
                    return Some(ProcessResult::Speak(format!(
                        "No albums found for {}",
                        artist
                    )));
                }
            }
        }

        None
    }

    /// Execute an Ollama-extracted intent
    ///
    /// Maps AI-detected intents to actual player commands
    pub async fn execute_ollama_intent(
        &self,
        intent: &crate::core::ollama::Intent,
    ) -> ProcessResult {
        info!(
            "🧠 Executing Ollama intent: {} (confidence: {:.2})",
            intent.name, intent.confidence
        );

        match intent.name.as_str() {
            "play_artist" => {
                if let Some(artist) = intent.parameters.get("artist") {
                    let _ = self.player.play_artist(artist).await;
                    return ProcessResult::Speak(format!("Playing {}", artist));
                }
            }
            "play_album" => {
                if let Some(album) = intent.parameters.get("album") {
                    let _ = self.player.play_album(album).await;
                    return ProcessResult::Speak(format!("Playing {}", album));
                }
            }
            "play_song" => {
                if let Some(song) = intent.parameters.get("song") {
                    let _ = self.player.play_song(song).await;
                    return ProcessResult::Speak(format!("Playing {}", song));
                }
            }
            "play_playlist" => {
                if let Some(playlist) = intent.parameters.get("playlist") {
                    let _ = self.player.play_playlist(playlist, false).await;
                    return ProcessResult::Speak(format!("Playing playlist {}", playlist));
                }
            }
            "media_control" => {
                if let Some(action) = intent.parameters.get("action") {
                    match action.as_str() {
                        "pause" | "play_pause" => {
                            let _ = self.player.play_pause().await;
                        }
                        "stop" => {
                            let _ = self.player.stop().await;
                        }
                        "next" => {
                            let _ = self.player.next_track().await;
                        }
                        "previous" => {
                            let _ = self.player.previous_track().await;
                        }
                        _ => {}
                    }
                    return ProcessResult::Handled;
                }
            }
            "volume_control" => {
                if let Some(action) = intent.parameters.get("action") {
                    match action.as_str() {
                        "up" | "louder" => {
                            let _ = self.player.volume_up().await;
                        }
                        "down" | "quieter" => {
                            let _ = self.player.volume_down().await;
                        }
                        _ => {}
                    }
                    return ProcessResult::Handled;
                }
            }
            "what_is_playing" => match self.player.what_is_playing().await {
                Ok(info) => return ProcessResult::Speak(info),
                Err(_) => {
                    return ProcessResult::Speak("Unable to determine what's playing".to_string())
                }
            },
            "game_command" => {
                // Game commands are delegated to game manager
                return ProcessResult::NotRecognized;
            }
            _ => {
                warn!("Unknown Ollama intent: {}", intent.name);
            }
        }

        ProcessResult::NotRecognized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quick_media_check() {
        assert!(Processor::quick_media_check("play beethoven"));
        assert!(Processor::quick_media_check("next track"));
        assert!(Processor::quick_media_check("volume up"));
        assert!(!Processor::quick_media_check("attack enemy"));
        assert!(!Processor::quick_media_check("open door"));
    }

    #[test]
    fn test_is_simple_control() {
        assert!(Processor::is_simple_control("pause"));
        assert!(Processor::is_simple_control("next"));
        assert!(Processor::is_simple_control("volume up"));
        assert!(!Processor::is_simple_control("play beethoven"));
    }

    #[test]
    fn test_is_quit_command() {
        assert!(Processor::is_quit_command("quit"));
        assert!(Processor::is_quit_command("goodbye"));
        assert!(Processor::is_quit_command("stop listening"));
        assert!(!Processor::is_quit_command("stop"));
    }
}
