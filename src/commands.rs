//! Command processing module
//!
//! Handles voice command matching and action execution.

use crate::core::ollama::{Intent, OllamaHandler};
use crate::input::{parse_key, VirtualKeyboard};
use crate::player_manager::PlayerManager;
use crate::utils::fuzzy::similarity;
use anyhow::Result;
use evdev::Key;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{debug, error, info, warn};

/// A step in a macro
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MacroStep {
    /// Action ID to execute (can be empty for audio-only steps)
    #[serde(default)]
    pub action: String,
    /// Delay in milliseconds after this step
    #[serde(default)]
    pub delay: u64,
    /// Optional sound pool to play during this step
    #[serde(default)]
    pub audio_pool: Vec<String>,
    /// How to play the sounds in the pool
    #[serde(default)]
    pub playback_mode: crate::audio::PlaybackMode,
    /// Legacy: single audio file path
    #[serde(default)]
    pub audio_feedback_file: Option<String>,
    /// LAL audio ID for content pack lookup
    #[serde(default)]
    pub audio_feedback: Option<String>,
}

/// A macro consisting of multiple steps
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Macro {
    pub name: String,
    pub triggers: Vec<String>,
    pub steps: Vec<MacroStep>,
}

/// A voice command binding
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum Command {
    /// Single key press action
    Action {
        name: String,
        triggers: Vec<String>,
        key: String,
        #[serde(default)]
        modifiers: Vec<String>,
    },
    /// Sequence of actions
    Macro(Macro),
}

/// Result of processing a voice command
#[derive(Debug, Clone)]
pub enum ProcessResult {
    /// Command executed successfully, returns action name
    Success(String),
    /// Command executed successfully after Ollama correction
    SuccessWithCorrection {
        action: String,
        original: String,
        corrected: String,
    },
    /// Multiple matches found, requires user selection
    SelectionRequired {
        query: String,
        results: Vec<crate::players::SearchResult>,
    },
    /// Requires verbal confirmation for high-risk action
    ConfirmationRequired { action: String, command: Command },
    /// No command matched
    NotFound,
}

/// Commands that require verbal confirmation for safety (Stamos requirement)
const DANGEROUS_COMMANDS: &[&str] = &[
    "self destruct",
    "eject",
    "abandon ship",
    "purge",
    "self-destruct",
];

/// Command processor that matches voice input to actions
#[derive(Clone)]
pub struct CommandProcessor {
    commands: Vec<Command>,
    pub keyboard: Arc<Mutex<Option<VirtualKeyboard>>>,
    /// Map of Action ID -> KeyBinding (populated by the active game profile)
    action_map: HashMap<String, crate::games::KeyBinding>,
    /// Audio engine for SFX
    pub sound_engine: Option<Arc<crate::audio::SoundEngine>>,
    /// Ollama Intent Handler
    pub ollama_handler: Option<OllamaHandler>,
    /// Player Manager for media control
    pub player_manager: Option<Arc<PlayerManager>>,
    /// LAL Manager for content packs
    pub lal_manager: Option<Arc<crate::lal::LALManager>>,
}

impl CommandProcessor {
    /// Create a new command processor
    pub fn new() -> Result<Self> {
        // Try to create virtual keyboard (requires uinput access)
        let keyboard = match VirtualKeyboard::new() {
            Ok(kb) => Some(kb),
            Err(e) => {
                warn!("⚠️ Could not create virtual keyboard: {}", e);
                warn!("   Run with: sudo ./tuxtalks or add user to 'input' group");
                None
            }
        };

        Ok(Self {
            commands: Vec::new(),
            keyboard: Arc::new(Mutex::new(keyboard)),
            action_map: HashMap::new(),
            sound_engine: None,
            ollama_handler: None,
            player_manager: None,
            lal_manager: None,
        })
    }

    pub fn set_lal_manager(&mut self, lal: Arc<crate::lal::LALManager>) {
        self.lal_manager = Some(lal);
    }

    /// Set the sound engine
    pub fn set_sound_engine(&mut self, engine: Arc<crate::audio::SoundEngine>) {
        self.sound_engine = Some(engine);
    }

    /// Set Ollama handler
    pub fn set_ollama_handler(&mut self, handler: OllamaHandler) {
        self.ollama_handler = Some(handler);
    }

    /// Set Player Manager
    pub fn set_player_manager(&mut self, manager: Arc<PlayerManager>) {
        self.player_manager = Some(manager);
    }

    /// Update the action map from the current game profile
    pub fn set_action_map(&mut self, map: HashMap<String, crate::games::KeyBinding>) {
        self.action_map = map;
    }

    /// Get current action map
    pub fn get_action_map(&self) -> HashMap<String, crate::games::KeyBinding> {
        self.action_map.clone()
    }

    /// Add a command
    pub fn add_command(&mut self, command: Command) {
        self.commands.push(command);
    }

    /// Add default demo bindings
    pub fn add_demo_bindings(&mut self) {
        self.add_command(Command::Action {
            name: "boost".into(),
            triggers: vec!["boost".into(), "boost engines".into()],
            key: "TAB".into(),
            modifiers: vec![],
        });
        self.add_command(Command::Action {
            name: "fire".into(),
            triggers: vec!["fire".into(), "shoot".into()],
            key: "SPACE".into(),
            modifiers: vec![],
        });
    }

    /// Match voice input to a command (without executing)
    pub fn match_command(&self, text: &str) -> Option<Command> {
        let text_lower = text.to_lowercase();

        // 1. Precise match
        if let Some(cmd) = self.commands.iter().find(|cmd| {
            let triggers = match cmd {
                Command::Action { triggers, .. } => triggers,
                Command::Macro(m) => &m.triggers,
            };
            triggers.iter().any(|t| text_lower.contains(t))
        }) {
            return Some(cmd.clone());
        }

        // 2. Phonetic fallback (Wendy Chisholm requirement)
        // Check if any trigger is phonetically similar to the input
        let mut best_match: Option<(f64, &Command)> = None;

        for cmd in &self.commands {
            let triggers = match cmd {
                Command::Action { triggers, .. } => triggers,
                Command::Macro(m) => &m.triggers,
            };

            for trigger in triggers {
                let score = similarity(&text_lower, trigger);
                if score > 0.7 {
                    // Lowered from 0.8 for better recall (Wendy Chisholm requirement)
                    if best_match.is_none() || score > best_match.as_ref().unwrap().0 {
                        best_match = Some((score, cmd));
                    }
                }
            }
        }

        best_match.map(|(_, cmd)| cmd.clone())
    }

    /// Process voice input with Triple-Layer Strategy
    pub async fn process(&mut self, text: &str) -> ProcessResult {
        let text_sanitized = sanitize_transcription(text);
        let text_lower = text_sanitized.to_lowercase();
        info!(
            "🔍 [Layer 0] Processing command: '{}' (original: '{}')",
            text_lower, text
        );

        // LAYER 1: Fast Keywords (Instant)
        if let Some(action) = self.check_fast_keywords(&text_lower).await {
            info!("⚡ Layer 1 (Fast Keyword) matched: {}", action);
            return ProcessResult::Success(action);
        }

        // LAYER 2: Existing Game Commands (Exact triggers)
        if let Some(cmd) = self.match_command(&text_sanitized) {
            let name = match &cmd {
                Command::Action { name, .. } => name.clone(),
                Command::Macro(m) => m.name.clone(),
            };

            info!("🎯 Layer 2 (Game Command) matched: {}", name);

            // Check for dangerous commands (Red Team: Stamos)
            if DANGEROUS_COMMANDS
                .iter()
                .any(|c| name.to_lowercase().contains(c))
            {
                info!(
                    "⚠️ Dangerous command detected: '{}', requesting confirmation",
                    name
                );
                return ProcessResult::ConfirmationRequired {
                    action: name,
                    command: cmd,
                };
            }

            let mut kb_lock = self.keyboard.lock().expect("Keyboard mutex poisoned");
            if let Err(e) = self.execute_command_blocking(&mut kb_lock, cmd) {
                warn!("❌ Failed to execute {}: {}", name, e);
                return ProcessResult::NotFound;
            }
            return ProcessResult::Success(name);
        }

        // LAYER 3: Ollama (Smart Intent)
        if let Some(ref handler) = self.ollama_handler {
            if handler.is_enabled() {
                debug!("🧠 Trying Ollama for: '{}'", text_sanitized);
                match handler
                    .extract_intent(&text_sanitized, &HashMap::new())
                    .await
                {
                    Ok(Some(intent)) => {
                        if intent.confidence > 0.6 && self.execute_intent(&intent).await {
                            if let Some((original, corrected)) =
                                handler.learn_from_success(text, &intent)
                            {
                                return ProcessResult::SuccessWithCorrection {
                                    action: intent.name,
                                    original,
                                    corrected,
                                };
                            }
                            return ProcessResult::Success(intent.name);
                        }
                    }
                    Ok(None) => debug!("Ollama returned no intent"),
                    Err(e) => warn!("Ollama error: {}", e),
                }
            }
        }

        // LAYER 4: Regex Fallback (Offline/Parity)
        debug!("🔍 Trying Layer 4 (Regex Fallback) for: '{}'", text_lower);
        let fallback_res = self.check_regex_fallback(&text_lower).await;
        if !matches!(fallback_res, ProcessResult::NotFound) {
            info!("📋 Layer 4 (Regex) matched something.");
            return fallback_res;
        }

        debug!("No command matched for: '{}'", text);
        ProcessResult::NotFound
    }

    // --- Layer 1: Fast Keywords ---
    async fn check_fast_keywords(&self, text: &str) -> Option<String> {
        let text = text.trim();

        if text == "stop" || text == "stop music" {
            if let Some(pm) = &self.player_manager {
                if let Err(e) = pm.player().read().await.stop().await {
                    warn!("Player error: {}", e);
                }
            }
            return Some("stop".into());
        }
        if text == "pause" || text == "resume" || text == "play" {
            if let Some(pm) = &self.player_manager {
                if let Err(e) = pm.player().read().await.play_pause().await {
                    warn!("Player error: {}", e);
                }
            }
            return Some("play_pause".into());
        }
        if text == "next" || text == "next track" || text == "next song" || text == "skip" {
            if let Some(pm) = &self.player_manager {
                if let Err(e) = pm.player().read().await.next_track().await {
                    warn!("Player error: {}", e);
                }
            }
            return Some("next_track".into());
        }
        if text == "previous" || text == "previous track" || text == "back" {
            if let Some(pm) = &self.player_manager {
                if let Err(e) = pm.player().read().await.previous_track().await {
                    warn!("Player error: {}", e);
                }
            }
            return Some("previous_track".into());
        }
        if text == "volume up" || text == "louder" {
            if let Some(pm) = &self.player_manager {
                if let Err(e) = pm.player().read().await.volume_up().await {
                    warn!("Player error: {}", e);
                }
            }
            return Some("volume_up".into());
        }
        if text == "volume down" || text == "quieter" {
            if let Some(pm) = &self.player_manager {
                if let Err(e) = pm.player().read().await.volume_down().await {
                    warn!("Player error: {}", e);
                }
            }
            return Some("volume_down".into());
        }
        if text == "what's playing" || text == "what is playing" {
            if let Some(pm) = &self.player_manager {
                // what_is_playing is String, not Result? It returns Option<String> or something.
                // Checking JRiverPlayer impl... usually Result<String> or Option<String>.
                let info = pm
                    .player()
                    .read()
                    .await
                    .what_is_playing()
                    .await
                    .unwrap_or_default();
                info!("🎵 Now Playing: {}", info);
                return Some("whats_playing".into());
            }
        }

        None
    }

    // --- Layer 3 Execution: Intents ---
    async fn execute_intent(&self, intent: &Intent) -> bool {
        match intent.name.as_str() {
            "play_artist" => {
                if let Some(artist) = intent.parameters.get("artist") {
                    if let Some(pm) = &self.player_manager {
                        // Guardrail: Verify entity exists (Wendy Chisholm requirement)
                        if !pm.library().artist_exists(artist) {
                            warn!(
                                "🔇 Entity verification failed: Artist '{}' not found",
                                artist
                            );
                            return false;
                        }
                        if let Err(e) = pm.player().read().await.play_artist(artist).await {
                            warn!("Player error: {}", e);
                        }
                    }
                    return true;
                }
            }
            "play_album" => {
                if let Some(album) = intent.parameters.get("album") {
                    if let Some(pm) = &self.player_manager {
                        if !pm.library().album_exists(album) {
                            warn!("🔇 Entity verification failed: Album '{}' not found", album);
                            return false;
                        }
                        if let Err(e) = pm.player().read().await.play_album(album).await {
                            warn!("Player error: {}", e);
                        }
                    }
                    return true;
                }
            }
            "play_song" => {
                if let Some(song) = intent.parameters.get("song") {
                    if let Some(pm) = &self.player_manager {
                        if !pm.library().song_exists(song) {
                            warn!("🔇 Entity verification failed: Song '{}' not found", song);
                            return false;
                        }
                        if let Err(e) = pm.player().read().await.play_song(song).await {
                            warn!("Player error: {}", e);
                        }
                    }
                    return true;
                }
            }
            "play_playlist" => {
                if let Some(playlist) = intent.parameters.get("playlist") {
                    if let Some(pm) = &self.player_manager {
                        if !pm.library().playlist_exists(playlist) {
                            warn!(
                                "🔇 Entity verification failed: Playlist '{}' not found",
                                playlist
                            );
                            return false;
                        }
                        if let Err(e) = pm
                            .player()
                            .read()
                            .await
                            .play_playlist(playlist, false)
                            .await
                        {
                            warn!("Player error: {}", e);
                        }
                    }
                    return true;
                }
            }
            "media_control" => {
                if let Some(action) = intent.parameters.get("action") {
                    return self.check_fast_keywords(action).await.is_some();
                }
            }
            _ => warn!("Unknown intent: {}", intent.name),
        }
        false
    }

    // --- Layer 4: Regex Fallback ---
    async fn check_regex_fallback(&self, text: &str) -> ProcessResult {
        let text_lower = text.trim().to_lowercase();
        let parts: Vec<&str> = text_lower.splitn(2, ' ').collect();
        let first_word = parts[0];
        let rest = if parts.len() > 1 { parts[1] } else { "" };
        debug!(
            "Regex Fallback: first_word='{}', rest='{}'",
            first_word, rest
        );

        // Fuzzy match the action verb "play"
        if similarity(first_word, "play") >= 0.6 {
            // It's a play command.

            // "play artist X"
            if rest.starts_with("artist ") {
                let artist_raw = rest.strip_prefix("artist ").unwrap_or_default().trim();
                let artist = artist_raw
                    .trim_start_matches("the ")
                    .trim_start_matches("a ")
                    .trim_start_matches("an ")
                    .trim();

                if let Some(pm) = &self.player_manager {
                    info!("🔎 [Layer 4] Play Artist: '{}'", artist);
                    let player = pm.player();
                    // Delegate to play_artist which should handle fuzzy matching internally
                    if let Err(e) = player.read().await.play_artist(artist).await {
                        warn!("Player error: {}", e);
                    };
                    return ProcessResult::Success(format!("play_artist_{}", artist));
                }
                return ProcessResult::NotFound;
            }

            // "play album X"
            if rest.starts_with("album ") {
                let album_raw = rest.strip_prefix("album ").unwrap_or_default().trim();
                let album = album_raw
                    .trim_start_matches("the ")
                    .trim_start_matches("a ")
                    .trim_start_matches("an ")
                    .trim();

                if let Some(pm) = &self.player_manager {
                    info!("🔎 [Layer 4] Play Album: '{}'", album);
                    let player = pm.player();
                    if let Err(e) = player.read().await.play_album(album).await {
                        warn!("Player error: {}", e);
                    };
                    return ProcessResult::Success(format!("play_album_{}", album));
                }
                return ProcessResult::NotFound;
            }

            // "play playlist X" or "X from playlist"
            if rest.starts_with("playlist ")
                || rest.contains("from playlist")
                || rest.starts_with("smartlist ")
                || rest.contains("from smartlist")
            {
                let playlist = rest
                    .replace("playlist ", "")
                    .replace("from playlist", "")
                    .replace("smartlist ", "")
                    .replace("from smartlist", "")
                    .trim()
                    .to_string();

                if let Some(pm) = &self.player_manager {
                    info!("🔎 [Layer 4] Play Playlist: '{}'", playlist);
                    let player = pm.player();
                    if let Err(e) = player.read().await.play_playlist(&playlist, false).await {
                        warn!("Player error: {}", e);
                    };
                    return ProcessResult::Success(format!("play_playlist_{}", playlist));
                }
                return ProcessResult::NotFound;
            }

            // "play whatever" (Random)
            if rest == "whatever" || rest == "random" || rest == "anything" {
                if let Some(pm) = &self.player_manager {
                    info!("🎲 [Layer 4] Play Whatever (Random)");
                    let player = pm.player();
                    if let Err(e) = player.read().await.play_pause().await {
                        // Fallback for random
                        warn!("Player error: {}", e);
                    }
                    return ProcessResult::Success("play_random".to_string());
                }
            }

            // "play X" (Generic)
            if !rest.is_empty() {
                // Strip common articles from query too (Vosk robustness)
                let query = rest
                    .trim_start_matches("the ")
                    .trim_start_matches("a ")
                    .trim_start_matches("an ")
                    .trim_start_matches("of ")
                    .trim_start_matches("by ")
                    .trim();

                if !query.is_empty() {
                    if let Some(pm) = &self.player_manager {
                        match pm.player().read().await.play_any(query).await {
                            Ok(results) => {
                                if results.is_empty() {
                                    warn!("❌ No generic matches found for '{}'", query);
                                    return ProcessResult::NotFound;
                                }

                                // DISAMBIGUATION: If multiple results, return for selection
                                if results.len() > 1 {
                                    info!(
                                        "🤔 Multiple matches for '{}', requesting selection...",
                                        query
                                    );
                                    return ProcessResult::SelectionRequired {
                                        query: query.to_string(),
                                        results,
                                    };
                                }

                                // Single result: auto-play
                                if let Some(best) = results.first() {
                                    info!(
                                        "✅ Auto-playing best match: {} ({})",
                                        best.display, best.score
                                    );
                                    let player_arc = pm.player();
                                    let player = player_arc.read().await;
                                    let _ = match best.result_type {
                                        crate::players::SearchResultType::Artist => {
                                            player.play_artist(&best.value).await
                                        }
                                        crate::players::SearchResultType::Album => {
                                            player.play_album(&best.value).await
                                        }
                                        crate::players::SearchResultType::Song => {
                                            player.play_song(&best.value).await
                                        }
                                        crate::players::SearchResultType::Playlist => {
                                            player.play_playlist(&best.value, false).await
                                        }
                                        crate::players::SearchResultType::Genre => {
                                            player.play_genre(&best.value).await
                                        }
                                    };
                                    return ProcessResult::Success(format!(
                                        "play_any_{}",
                                        best.value
                                    ));
                                }
                            }
                            Err(e) => {
                                warn!("Player error: {}", e);
                                return ProcessResult::NotFound;
                            }
                        }
                    }
                    return ProcessResult::Success(format!("play_any_{}", query));
                }
            }
        }

        ProcessResult::NotFound
    }

    /// Execute a command in a blocking manner (legacy)
    pub fn execute_command_blocking(
        &self,
        keyboard: &mut Option<VirtualKeyboard>,
        command: Command,
    ) -> Result<()> {
        match command {
            Command::Action { key, modifiers, .. } => {
                self.press_keys_internal_opt(keyboard, &key, &modifiers)
            }
            Command::Macro(m) => {
                info!("📜 Executing macro: {}", m.name);
                for (step_idx, step) in m.steps.iter().enumerate() {
                    // === Audio Fallback Chain (matching Python) ===
                    if let Some(engine) = &self.sound_engine {
                        let mut audio_pool = step.audio_pool.clone();

                        // Legacy fallback
                        if audio_pool.is_empty() {
                            if let Some(ref legacy_file) = step.audio_feedback_file {
                                if legacy_file != "(Sound Pool)" {
                                    audio_pool.push(legacy_file.clone());
                                }
                            }
                        }

                        // Play audio
                        if !audio_pool.is_empty() {
                            let pool_id = format!("{}_step_{}", m.name, step_idx);
                            let paths = audio_pool.iter().map(std::path::PathBuf::from).collect();
                            let _ = engine.play_pool(&pool_id, paths, step.playback_mode);
                        }
                    }

                    // Execute key press (only if action is specified)
                    if !step.action.is_empty() {
                        let binding = self.action_map.get(&step.action).cloned();

                        if let Some(binding) = binding {
                            if let Some(key) = &binding.primary_key {
                                self.press_keys_internal_opt(keyboard, key, &binding.modifiers)?;
                            }
                        } else {
                            warn!("⚠️ Unknown action in macro: {}", step.action);
                        }
                    }

                    if step.delay > 0 {
                        std::thread::sleep(std::time::Duration::from_millis(step.delay));
                    }
                }
                Ok(())
            }
        }
    }

    /// Execute a command asynchronously (for use in Tasks)
    pub async fn execute_command_async(
        shared_keyboard: Arc<Mutex<Option<VirtualKeyboard>>>,
        action_map: HashMap<String, crate::games::KeyBinding>,
        sound_engine: Option<Arc<crate::audio::SoundEngine>>,
        lal_manager: Option<Arc<crate::lal::LALManager>>,
        custom_audio_dir: Option<std::path::PathBuf>,
        command: Command,
    ) {
        match command {
            Command::Action {
                key,
                modifiers,
                name,
                ..
            } => {
                info!(
                    "⚡ Executing Action: {} (Key: {}, Modifiers: {:?})",
                    name, key, modifiers
                );
                let mut kb = shared_keyboard
                    .lock()
                    .expect("Shared keyboard mutex poisoned");
                if let Some(ref mut k) = *kb {
                    if let Err(e) = Self::press_keys_internal(k, &key, &modifiers) {
                        error!("❌ Failed to press keys: {}", e);
                    }
                } else {
                    warn!("⚠️ No virtual keyboard available for action execution");
                }
            }
            Command::Macro(m) => {
                info!("📜 Executing macro (async): {}", m.name);
                for (step_idx, step) in m.steps.iter().enumerate() {
                    // === Audio Fallback Chain (matching Python) ===
                    // 1. Check audio_pool
                    // 2. If empty, check audio_feedback_file (legacy single file)
                    // 3. If empty, check audio_feedback (LAL ID)

                    if let Some(engine) = &sound_engine {
                        let mut audio_pool = step.audio_pool.clone();

                        // Legacy fallback: if no pool, use single file
                        if audio_pool.is_empty() {
                            if let Some(ref legacy_file) = step.audio_feedback_file {
                                if legacy_file != "(Sound Pool)" {
                                    audio_pool.push(legacy_file.clone());
                                }
                            }
                        }

                        // LAL ID fallback: if still empty and ID exists
                        if audio_pool.is_empty() {
                            if let Some(ref audio_id) = step.audio_feedback {
                                if let Some(mgr) = &lal_manager {
                                    if let Some(path) = mgr.get_audio(audio_id) {
                                        let path_str = path.to_string_lossy().to_string();
                                        audio_pool.push(path_str);
                                    } else {
                                        warn!("⚠️ LAL Audio ID not found: {}", audio_id);
                                    }
                                }
                            }
                        }

                        // Play audio
                        if !audio_pool.is_empty() {
                            let pool_id = format!("{}_step_{}", m.name, step_idx);
                            let mut resolved_paths = Vec::new();

                            for p in audio_pool {
                                let path = std::path::PathBuf::from(&p);
                                let mut final_path = path.clone();

                                // 1. Check if path exists directly (or relative to CWD)
                                if !final_path.exists() {
                                    // 2. Check Custom Audio Dir
                                    if let Some(ref custom_dir) = custom_audio_dir {
                                        let custom_path = custom_dir.join(&p);
                                        if custom_path.exists() {
                                            final_path = custom_path;
                                        }
                                    }
                                }

                                // 3. Directory Randomization (Task 7.5)
                                if final_path.is_dir() {
                                    if let Ok(entries) = std::fs::read_dir(&final_path) {
                                        let files: Vec<_> = entries
                                            .filter_map(|e| e.ok())
                                            .map(|e| e.path())
                                            .filter(|p| {
                                                p.is_file()
                                                    && p.extension().is_some_and(|ext| {
                                                        let ext = ext.to_string_lossy();
                                                        matches!(
                                                            ext.as_ref(),
                                                            "wav" | "mp3" | "ogg" | "flac"
                                                        )
                                                    })
                                            })
                                            .collect();

                                        if !files.is_empty() {
                                            use rand::seq::SliceRandom;
                                            if let Some(picked) =
                                                files.choose(&mut rand::thread_rng())
                                            {
                                                final_path = picked.clone();
                                                debug!(
                                                    "🎲 Randomly picked {} from {}",
                                                    final_path.display(),
                                                    p
                                                );
                                            }
                                        }
                                    }
                                }

                                if final_path.exists() && final_path.is_file() {
                                    resolved_paths.push(final_path);
                                } else {
                                    warn!("⚠️ Audio file not found: {}", p);
                                }
                            }

                            if !resolved_paths.is_empty() {
                                let _ =
                                    engine.play_pool(&pool_id, resolved_paths, step.playback_mode);
                            }
                        }
                    }

                    // Execute key press (only if action is specified)
                    if !step.action.is_empty() {
                        let binding = action_map.get(&step.action).cloned();

                        if let Some(binding) = binding {
                            if let Some(key) = &binding.primary_key {
                                let mut kb = shared_keyboard
                                    .lock()
                                    .expect("Shared keyboard mutex poisoned");
                                if let Some(ref mut k) = *kb {
                                    let _ = Self::press_keys_internal(k, key, &binding.modifiers);
                                }
                            }
                        } else {
                            warn!("⚠️ Unknown action in macro: {}", step.action);
                        }
                    }

                    if step.delay > 0 {
                        tokio::time::sleep(std::time::Duration::from_millis(step.delay)).await;
                    }
                }
            }
        }
    }

    fn press_keys_internal_opt(
        &self,
        keyboard: &mut Option<VirtualKeyboard>,
        key_str: &str,
        modifier_strs: &[String],
    ) -> Result<()> {
        let k = keyboard
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("No keyboard"))?;
        Self::press_keys_internal(k, key_str, modifier_strs)
    }

    fn press_keys_internal(
        keyboard: &mut VirtualKeyboard,
        key_str: &str,
        modifier_strs: &[String],
    ) -> Result<()> {
        let key = parse_key(key_str).ok_or_else(|| anyhow::anyhow!("Unknown key: {}", key_str))?;

        if modifier_strs.is_empty() {
            keyboard.tap_key(key)?;
        } else {
            let modifiers: Vec<Key> = modifier_strs.iter().filter_map(|m| parse_key(m)).collect();
            keyboard.key_combo(&modifiers, key)?;
        }

        debug!("⌨️ Pressed: {:?} + {:?}", modifier_strs, key_str);
        Ok(())
    }

    /// Check if we have a working keyboard
    pub fn has_keyboard(&self) -> bool {
        self.keyboard
            .lock()
            .expect("Keyboard mutex poisoned")
            .is_some()
    }
}

/// Sanitize transcription by stripping junk prefixes and punctuation
fn sanitize_transcription(text: &str) -> String {
    let mut s = text.to_lowercase();

    // Strip common filler prefixes (Parity with Python TextNormalizer)
    let fillers = ["the ", "and ", "a ", "an ", "to "];
    for filler in fillers {
        if s.starts_with(filler) {
            s = s.strip_prefix(filler).unwrap().to_string();
            break; // Only strip one prefix
        }
    }

    s.trim_matches(|c: char| c.is_ascii_punctuation() || c.is_whitespace())
        .to_string()
}
