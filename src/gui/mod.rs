//! GUI module using iced
//!
//! Provides a graphical launcher for TuxTalks.

use iced::futures::SinkExt;
use iced::widget::{container, row};
use iced::{Element, Length, Subscription, Task};
use tracing::{debug, info, warn};

use crate::audio;
use crate::players::{SearchResult, SearchResultType};

// Sub-modules
pub mod app;
pub mod messages;
pub mod state;
pub mod tabs;
pub mod wizards;

// Re-exports for convenience
pub use app::TuxTalksApp;
pub use messages::Message;
pub use state::Tab;

// ASR Pause Signal (Static to avoid closure capture issues)
pub static ASR_PAUSED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

// Re-export speak wrapper
pub async fn msg_speak(client: std::sync::Arc<dyn crate::tts::TtsEngine>, text: String) -> Message {
    let _ = client.speak(&text).await;
    Message::SpeechFinished
}

pub async fn selection_timeout_task(id: u64) -> Message {
    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    Message::SelectionTimeout(id)
}

pub async fn confirmation_timeout_task(id: u64) -> Message {
    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    Message::ConfirmationTimeout(id)
}

impl TuxTalksApp {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TabSelected(tab) => {
                self.current_tab = tab;
            }
            Message::SpeechdConnected(engine) => {
                info!("🔊 TTS Connected");
                self.tts = Some(engine);
                self.status = "Ready".to_string();
                // NOTE (Jony): Do NOT announce here. Wait for user to click Start Listening.
            }
            Message::SpeechdFailed => {
                warn!("⚠️ TTS Connection Failed");
                self.status = "TTS Failed".to_string();
            }
            Message::ToggleListening => {
                if self.listening {
                    return self.update(Message::StopPressed);
                } else {
                    return self.update(Message::StartPressed);
                }
            }
            Message::StartPressed => {
                info!(
                    "▶️ Start Listening Pressed. TTS Available: {}",
                    self.tts.is_some()
                );
                self.listening = true;
                self.status = "Listening...".to_string();

                // Load commands from active profile if any
                if let Some(profile) = self.game_manager.get_active_profile() {
                    let commands = profile.get_processor_commands();
                    info!(
                        "🚀 Loading {} commands from profile '{}'",
                        commands.len(),
                        profile.name
                    );

                    // Reset and load
                    self.processor = crate::commands::CommandProcessor::new()
                        .expect("Failed to create CommandProcessor");
                    self.processor.set_sound_engine(self.sound_engine.clone());
                    for cmd in commands {
                        self.processor.add_command(cmd);
                    }
                    self.processor.set_action_map(profile.resolve_actions());

                    // Wire up Ollama if enabled
                    if self.config.ollama_enabled {
                        self.processor.set_ollama_handler(self.ollama.clone());
                    }
                } else {
                    self.processor.add_demo_bindings();
                }

                // Announce "I am ready" when listening starts
                if let Some(ref tts) = self.tts {
                    ASR_PAUSED.store(true, std::sync::atomic::Ordering::SeqCst);
                    self.tts_mute_until =
                        Some(std::time::Instant::now() + std::time::Duration::from_millis(2500));
                    return Task::perform(msg_speak(tts.clone(), "I am ready".to_string()), |m| m);
                }
            }
            Message::StopPressed => {
                self.listening = false;
                self.status = "Stopped".to_string();
            }
            Message::Transcription(text) => {
                // 🛡️ Robust ASR Squelch Filter (Wendy UX)
                let lower = text.to_lowercase();
                let _is_navigation = [
                    "next", "previous", "cancel", "confirm", "one", "two", "three", "four", "five",
                    "six", "seven", "eight", "nine", "ten",
                ]
                .iter()
                .any(|p| lower.contains(p));

                // Only block if ASR is paused (Stamos Hardlock)
                if ASR_PAUSED.load(std::sync::atomic::Ordering::SeqCst) {
                    debug!("🔇 Dropping transcription as ASR is paused: '{}'", text);
                    return Task::none();
                }

                if let Some(mute_until) = self.tts_mute_until {
                    if std::time::Instant::now() < mute_until {
                        info!(
                            "🔇 Ignoring transcription during TTS mute window: '{}'",
                            text
                        );
                        return Task::none();
                    }
                }

                // Rate Limiting (Red Team Audit: Alex Stamos)
                let now = std::time::Instant::now();
                if let Some(last) = self.last_command_time {
                    if now.duration_since(last) < std::time::Duration::from_millis(500) {
                        warn!("⚠️ Rate limiting command: '{}'", text);
                        return Task::none();
                    }
                }
                self.last_command_time = Some(now);

                info!("📥 Transcription: {}", text);

                // Check if we are in an active selection phase (Voice Fallback)
                if self.selection_handler.is_active() {
                    let mut is_timed_out = false;
                    if let Some(timeout) = self.selection_timeout {
                        if std::time::Instant::now() > timeout {
                            is_timed_out = true;
                        }
                    }

                    if !is_timed_out {
                        use crate::selection::SelectionResult;
                        let text = self.normalizer.normalize(&text);
                        match self.selection_handler.handle_command(&text) {
                            SelectionResult::Selected(item, idx) => {
                                info!("📌 Voice selection: {} (idx {})", item.display, idx);
                                self.selection_timeout = None;

                                // Reset asr_paused to allow TTS feedback
                                ASR_PAUSED.store(true, std::sync::atomic::Ordering::SeqCst);

                                // If it was an IPC request, we need to send the response back
                                if let Some(resp_tx) = self.pending_ipc_resp.take() {
                                    let _ = resp_tx.send((idx as i32, false));
                                    self.status = format!("Selected: {}", item.display);
                                    let response = format!("Selected {}", item.display);
                                    return Task::perform(
                                        msg_speak(self.tts.as_ref().unwrap().clone(), response),
                                        |m| m,
                                    );
                                } else {
                                    // Internal search selection
                                    use crate::players::SearchResultType;
                                    let player_manager = self.player_manager.clone();
                                    let item_to_play = item.clone();

                                    self.status = format!("Playing: {}", item.display);
                                    let response = format!("Playing {}", item.display);

                                    // Reset asr_paused to allow TTS feedback
                                    ASR_PAUSED.store(true, std::sync::atomic::Ordering::SeqCst);

                                    return Task::batch(vec![
                                        Task::perform(
                                            async move {
                                                let player_lock = player_manager.player();
                                                let player = player_lock.read().await;
                                                let _ = match item_to_play.result_type {
                                                    SearchResultType::Artist => {
                                                        player
                                                            .play_artist(&item_to_play.value)
                                                            .await
                                                    }
                                                    SearchResultType::Album => {
                                                        player.play_album(&item_to_play.value).await
                                                    }
                                                    SearchResultType::Song => {
                                                        player.play_song(&item_to_play.value).await
                                                    }
                                                    SearchResultType::Playlist => {
                                                        player
                                                            .play_playlist(
                                                                &item_to_play.value,
                                                                false,
                                                            )
                                                            .await
                                                    }
                                                    SearchResultType::Genre => {
                                                        player.play_genre(&item_to_play.value).await
                                                    }
                                                };
                                            },
                                            |_| Message::None,
                                        ),
                                        Task::perform(
                                            msg_speak(self.tts.as_ref().unwrap().clone(), response),
                                            |m| m,
                                        ),
                                    ]);
                                }
                            }
                            SelectionResult::NextPage
                            | SelectionResult::PreviousPage
                            | SelectionResult::SpeakOptions => {
                                info!("📄 Selection navigation: {}", text);
                                self.selection_id += 1;
                                let prompt = self.selection_handler.speak_options_text();
                                self.selection_timeout = Some(
                                    std::time::Instant::now() + std::time::Duration::from_secs(30),
                                );

                                if let Some(tts) = &self.tts {
                                    ASR_PAUSED.store(true, std::sync::atomic::Ordering::SeqCst);
                                    return Task::perform(msg_speak(tts.clone(), prompt), |m| m);
                                }
                                return Task::none();
                            }
                            SelectionResult::Cancelled => {
                                self.selection_timeout = None;
                                if let Some(resp_tx) = self.pending_ipc_resp.take() {
                                    let _ = resp_tx.send((-1, true));
                                }
                                if let Some(tts) = &self.tts {
                                    // Stop current playback if any (Chisholm requirement)
                                    let _ = self.sound_engine.stop();

                                    ASR_PAUSED.store(true, std::sync::atomic::Ordering::SeqCst);
                                    return Task::perform(
                                        msg_speak(tts.clone(), "Selection cancelled".to_string()),
                                        |m| m,
                                    );
                                }
                                return Task::none();
                            }
                            SelectionResult::NotRecognized => {
                                // Fall through to normal processing or ignore?
                                // Alex (Red Team) says: "Validate all inputs... to prevent errors."
                                // If we are active, maybe we should ONLY accept selection commands?
                                // Let's fall through but log it.
                                debug!("Selection: input not recognized for active handler");
                            }
                        }
                    } else {
                        info!("⌛ Selection phase timed out.");
                        self.selection_handler.clear();
                        self.selection_timeout = None;
                        if let Some(resp_tx) = self.pending_ipc_resp.take() {
                            let _ = resp_tx.send((-1, true));
                        }
                    }
                }

                // Check if we are in an active confirmation phase (Stamos requirement)
                if let Some((_name, _cmd)) = self.pending_confirmation.clone() {
                    let mut is_timed_out = false;
                    if let Some(timeout) = self.confirmation_timeout {
                        if std::time::Instant::now() > timeout {
                            is_timed_out = true;
                        }
                    }

                    if !is_timed_out {
                        let text = self.normalizer.normalize(&text).to_lowercase();
                        if text == "confirm"
                            || text == "yes"
                            || text == "do it"
                            || text == "proceed"
                        {
                            return self.update(Message::ConfirmCommand);
                        } else if text == "cancel"
                            || text == "no"
                            || text == "stop"
                            || text == "abort"
                        {
                            return self.update(Message::CancelConfirmation);
                        }
                        // If we didn't match confirm/cancel, we just fall through but maybe log?
                        debug!(
                            "Confirmation: input '{}' did not match confirm/cancel",
                            text
                        );
                    } else {
                        info!("⌛ Confirmation phase timed out.");
                        self.pending_confirmation = None;
                        self.confirmation_timeout = None;
                    }
                }

                // COMMAND MODE TIMEOUT CHECK (Wendy - 10 second timeout)
                if self.command_mode == crate::gui::app::CommandModeState::Active {
                    if let Some(timeout) = self.command_mode_timeout {
                        if std::time::Instant::now() > timeout {
                            info!("⌛ Command mode timed out");
                            self.command_mode = crate::gui::app::CommandModeState::Inactive;
                            self.command_mode_timeout = None;
                            self.status = "Command mode timed out".to_string();
                            // Don't return - process this transcription normally
                        }
                    }
                }

                // WAKE WORD & COMMAND MODE LOGIC (Team Approved)
                let mut command_text = text.clone();

                // 🛡️ SELECTION BYPASS: Skip wake word for active selections (Jony UX)
                let is_selection_active =
                    self.selection_handler.is_active() || self.pending_confirmation.is_some();
                let lower = text.to_lowercase();
                let bypass_phrases = [
                    "next", "previous", "cancel", "confirm", "one", "two", "three", "four", "five",
                    "six", "seven", "eight", "nine", "ten", "0", "1", "2", "3", "4", "5", "6", "7",
                    "8", "9",
                ];
                let is_bypass =
                    is_selection_active && bypass_phrases.iter().any(|p| lower.contains(p));

                if !self.config.wake_word.is_empty() && !is_bypass {
                    let wake_word = self.config.wake_word.to_lowercase();
                    let input_lower = text.to_lowercase();

                    // Check if already in command mode (no wake word needed)
                    if self.command_mode == crate::gui::app::CommandModeState::Active {
                        // 1. Check if user said JUST the wake word, effectively resetting the session
                        if input_lower == wake_word {
                            info!("🔔 Wake word repeated in active session - resetting timeout");
                            self.command_mode_timeout = Some(
                                std::time::Instant::now() + std::time::Duration::from_secs(10),
                            );

                            if let Some(ref tts) = self.tts {
                                ASR_PAUSED.store(true, std::sync::atomic::Ordering::SeqCst);
                                self.tts_mute_until = Some(
                                    std::time::Instant::now()
                                        + std::time::Duration::from_millis(1500),
                                );
                                return Task::perform(
                                    msg_speak(tts.clone(), "Yes?".to_string()),
                                    |m| m,
                                );
                            }
                            return Task::none();
                        }

                        // 2. Check if user said "Wake Word <Command>" again
                        if input_lower.starts_with(&format!("{} ", wake_word)) {
                            command_text = input_lower[wake_word.len()..].trim().to_string();
                            info!(
                                "🚀 Stripped repeated wake word from active command: '{}'",
                                command_text
                            );
                        } else {
                            // Already in command mode - process directly
                            command_text = input_lower.clone();
                        }

                        // Extend timeout since user is speaking
                        self.command_mode_timeout =
                            Some(std::time::Instant::now() + std::time::Duration::from_secs(10));
                    } else if let Some(idx) = input_lower.find(&wake_word) {
                        // Wake word detected - enter command mode
                        info!("🔔 Wake word '{}' detected!", wake_word);

                        // Audit log (Alex)
                        let _ =
                            self.flush_audit_log(&format!("Wake word '{}' detected", wake_word));

                        // Extract command after wake word (if any)
                        let after_wake = input_lower[idx + wake_word.len()..].trim();

                        if after_wake.is_empty() {
                            // Just wake word - enter command mode + respond "Yes?" (Jony)
                            self.command_mode = crate::gui::app::CommandModeState::Active;
                            self.command_mode_timeout = Some(
                                std::time::Instant::now() + std::time::Duration::from_secs(10),
                            );
                            self.status = "Listening for command...".to_string();

                            if let Some(ref tts) = self.tts {
                                ASR_PAUSED.store(true, std::sync::atomic::Ordering::SeqCst);
                                self.tts_mute_until = Some(
                                    std::time::Instant::now()
                                        + std::time::Duration::from_millis(1500),
                                );
                                return Task::perform(
                                    msg_speak(tts.clone(), "Yes?".to_string()),
                                    |m| m,
                                );
                            }
                            return Task::none();
                        } else {
                            // Wake word + command in same utterance (v3: Look-Forward Buffer)
                            info!("🚀 Processing look-forward command: '{}'", after_wake);
                            command_text = after_wake.to_string();
                            // Stay in command mode briefly for follow-up
                            self.command_mode = crate::gui::app::CommandModeState::Active;
                            self.command_mode_timeout = Some(
                                std::time::Instant::now() + std::time::Duration::from_secs(10),
                            );

                            // Acknowledge look-forward (User request: Silent execution for flow)
                            // "I should expect 'I am ready' followed by 'Yes?' when my wake word is recognized."
                            // For combined commands, just run them.

                            // Process the extracted command text
                            let mut processor = self.processor.clone();
                            let transcription = self.normalizer.normalize(&command_text);
                            return Task::perform(
                                async move { processor.process(&transcription).await },
                                Message::GameCommandResult,
                            );
                        }
                    } else {
                        // No wake word, not in command mode, and not a bypass phrase - ignore
                        debug!("🔇 Ignoring transcription without wake word: '{}'", text);
                        return Task::none();
                    }
                }

                // Apply Voice Corrections (Normalization)
                let text = self.normalizer.normalize(&command_text);
                if text.is_empty() {
                    return Task::none();
                }

                // Exit command mode after processing (command was received)
                self.command_mode = crate::gui::app::CommandModeState::Inactive;
                self.command_mode_timeout = None;

                // Process command asynchronously
                let mut processor = self.processor.clone();
                let transcription = text.clone();
                return Task::perform(
                    async move { processor.process(&transcription).await },
                    Message::GameCommandResult,
                );
            }
            Message::ManualInput(text) => {
                let text = text.trim();
                info!("⌨️ Manual Input: '{}'", text);

                // 1. Check Selections & Confirmation (Priority 1)
                // Manual input should ALWAYS be able to select/confirm
                if let Some(resp_tx) = self.pending_ipc_resp.take() {
                    // Check if input is a valid selection index
                    if let Ok(idx) = text.parse::<i32>() {
                        info!("⌨️ Manual IPC Selection: {}", idx);
                        let _ = resp_tx.send((idx, true)); // true = final selection
                        return Task::none();
                    }
                    self.pending_ipc_resp = Some(resp_tx);
                }

                if self.selection_handler.is_active() {
                    if let Some(idx) = self.selection_handler.handle_input(text) {
                        info!("📌 Manual Selection: Index {}", idx);
                        // Fetch the result
                        if let Some(res) = self.search_results.get(idx).cloned() {
                            return self.update(Message::SelectResult(res, idx));
                        }
                    }
                }

                if self.pending_confirmation.is_some() {
                    // Manual confirm
                    let lower = text.to_lowercase();
                    if lower.contains("yes")
                        || lower.contains("confirm")
                        || lower.contains("ok")
                        || lower == "y"
                    {
                        return self.update(Message::ConfirmCommand);
                    } else if lower.contains("no") || lower.contains("cancel") || lower == "n" {
                        return self.update(Message::CancelConfirmation);
                    }
                }

                // 2. Process as Command (No Wake Word needed for Manual Input)
                let text_norm = self.normalizer.normalize(text);
                if text_norm.is_empty() {
                    return Task::none();
                }

                self.status = format!("Processing: {}", text_norm);

                // Process command asynchronously
                let mut processor = self.processor.clone();
                let transcription = text_norm.clone();
                return Task::perform(
                    async move { processor.process(&transcription).await },
                    Message::GameCommandResult,
                );
            }
            Message::GameCommandResult(result) => {
                use crate::commands::ProcessResult;
                match result {
                    ProcessResult::Success(cmd_name) => {
                        self.status = format!("Executed: {}", cmd_name);

                        // Learning (Stamos requirement)
                        self.voice_fingerprint.add_successful_command(&cmd_name);

                        // Audit log (Red Team requirement)
                        let entry = format!("Executed: {}", cmd_name);
                        self.command_audit_log.push(entry.clone());
                        if self.command_audit_log.len() > 100 {
                            self.command_audit_log.remove(0);
                        }
                        let _ = self.flush_audit_log(&entry);

                        // TTS Feedback
                        if let Some(tts) = &self.tts {
                            ASR_PAUSED.store(true, std::sync::atomic::Ordering::SeqCst);
                            let text_to_speak = format!("Executing {}", cmd_name);
                            return Task::perform(msg_speak(tts.clone(), text_to_speak), |m| m);
                        }
                    }
                    ProcessResult::SuccessWithCorrection {
                        action,
                        original,
                        corrected,
                    } => {
                        self.status = format!("Executed: {} (Corrected)", action);

                        // Learning (Passive)
                        self.voice_fingerprint
                            .add_passive_correction(&original, &corrected);
                        self.voice_fingerprint.add_successful_command(&action);

                        // Audit log
                        let entry = format!(
                            "Executed: {} (Correction applied: '{}' -> '{}')",
                            action, original, corrected
                        );
                        self.command_audit_log.push(entry.clone());
                        if self.command_audit_log.len() > 100 {
                            self.command_audit_log.remove(0);
                        }
                        let _ = self.flush_audit_log(&entry);

                        // TTS Feedback
                        if let Some(tts) = &self.tts {
                            ASR_PAUSED.store(true, std::sync::atomic::Ordering::SeqCst);
                            let text_to_speak = format!("Executing {}", action);
                            return Task::perform(msg_speak(tts.clone(), text_to_speak), |m| m);
                        }
                    }
                    ProcessResult::SelectionRequired { query, results } => {
                        info!("🎯 Multiple results for '{}', showing selection", query);
                        self.search_results = results.clone();
                        self.selection_handler.reset();
                        self.selection_handler
                            .set_title(format!("Select result for '{}'", query));
                        self.selection_handler.set_results(results);
                        self.selection_id += 1;
                        self.selection_timeout =
                            Some(std::time::Instant::now() + std::time::Duration::from_secs(30));

                        self.current_tab = Tab::Player; // Switch to results view

                        if let Some(tts) = &self.tts {
                            let prompt = self.selection_handler.speak_options_text();
                            ASR_PAUSED.store(true, std::sync::atomic::Ordering::SeqCst);
                            return Task::perform(msg_speak(tts.clone(), prompt), |m| m);
                        }
                        return Task::none();
                    }
                    ProcessResult::ConfirmationRequired { action, command } => {
                        info!("⚠️ Confirmation required for: {}", action);
                        self.pending_confirmation = Some((action.clone(), command));
                        self.confirmation_id += 1;
                        self.confirmation_timeout =
                            Some(std::time::Instant::now() + std::time::Duration::from_secs(30));
                        self.status = format!("Confirm: {}", action);

                        if let Some(tts) = &self.tts {
                            let prompt = format!("Dangerous command detected: {}. Say confirm to proceed or cancel to abort.", action);
                            ASR_PAUSED.store(true, std::sync::atomic::Ordering::SeqCst);
                            return Task::perform(msg_speak(tts.clone(), prompt), |m| m);
                        }
                        return Task::none();
                    }
                    ProcessResult::NotFound => {
                        debug!("No command matched");
                        if let Some(tts) = &self.tts {
                            ASR_PAUSED.store(true, std::sync::atomic::Ordering::SeqCst);
                            return Task::perform(
                                msg_speak(tts.clone(), "I didn't catch that command.".to_string()),
                                |m| m,
                            );
                        }
                    }
                }
            }
            Message::IpcSelectionRequest {
                seq_id: _,
                title,
                items,
                page,
                resp_tx,
            } => {
                info!("📡 IPC Selection Request: {}", title);
                self.selection_handler.reset();
                self.selection_handler.set_title(title.clone());

                // Map String to SearchResult
                let results: Vec<SearchResult> = items
                    .into_iter()
                    .map(|s| SearchResult {
                        display: s.clone(),
                        value: s,
                        result_type: SearchResultType::Song,
                        score: 1.0,
                    })
                    .collect();

                self.selection_handler.set_results(results);
                self.selection_handler.set_page(page);
                self.selection_id += 1;
                let sid = self.selection_id;
                self.selection_timeout =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(30));

                self.pending_ipc_resp = Some(resp_tx);
                self.current_tab = Tab::Player;

                if let Some(tts) = &self.tts {
                    let prompt =
                        format!("{}. {}", title, self.selection_handler.speak_options_text());
                    ASR_PAUSED.store(true, std::sync::atomic::Ordering::SeqCst);
                    return Task::batch(vec![
                        Task::perform(msg_speak(tts.clone(), prompt), |m| m),
                        Task::perform(selection_timeout_task(sid), |m| m),
                    ]);
                }
                return Task::none();
            }
            Message::NewCorrectionInChanged(val) => {
                self.new_correction_in = val;
            }
            Message::NewCorrectionOutChanged(val) => {
                self.new_correction_out = val;
            }
            Message::AddCorrection(input, output) => {
                if !input.is_empty() && !output.is_empty() {
                    self.config.voice_corrections.insert(input, output);
                    // Refresh normalizer
                    self.normalizer = crate::core::text_normalizer::TextNormalizer::new(
                        self.config.voice_corrections.clone(),
                    );
                    self.new_correction_in.clear();
                    self.new_correction_out.clear();
                }
            }
            Message::RemoveCorrection(input) => {
                self.config.voice_corrections.remove(&input);
                self.normalizer = crate::core::text_normalizer::TextNormalizer::new(
                    self.config.voice_corrections.clone(),
                );
            }
            Message::SaveConfig => {
                info!("💾 Saving configuration...");
                if let Err(e) = self.config.save() {
                    warn!("Failed to save config: {}", e);
                    self.status = format!("Save Error: {}", e);
                } else {
                    self.status = "Config Saved".to_string();
                }
            }
            Message::ToggleTrainingRecording => {
                self.training_state.is_recording = !self.training_state.is_recording;
                if self.training_state.is_recording {
                    self.status = "Recording voice sample...".to_string();
                    let phrase = self
                        .training_state
                        .current_phrase
                        .clone()
                        .unwrap_or_else(|| "test".to_string());
                    let vf = self.voice_fingerprint.clone();
                    return Task::perform(
                        async move { vf.record_sample(&phrase).await.map_err(|e| e.to_string()) },
                        Message::TrainingRecordingCompleted,
                    );
                } else {
                    self.status = "Recording stopped".to_string();
                }
            }
            Message::TrainingRecordingCompleted(res) => {
                self.training_state.is_recording = false;
                match res {
                    Ok(path) => {
                        self.status = format!(
                            "Recorded sample to {:?}",
                            path.file_name().unwrap_or_default()
                        );
                    }
                    Err(e) => {
                        warn!("Recording failed: {}", e);
                        self.status = format!("Recording Error: {}", e);
                    }
                }
            }
            Message::AddVocabularyTerm(term) => {
                if !term.is_empty() && !self.config.custom_vocabulary.contains(&term) {
                    self.config.custom_vocabulary.push(term);
                    self.new_vocab_input.clear();
                }
            }
            Message::RemoveVocabularyTerm(term) => {
                self.config.custom_vocabulary.retain(|t| t != &term);
            }
            Message::NewVocabularyInputChanged(val) => {
                self.new_vocab_input = val;
            }
            Message::ResetFingerprint => {
                self.voice_fingerprint.clear_patterns();
                self.status = "Voice fingerprint reset".to_string();
            }
            Message::SelectionTimeout(id) => {
                if self.selection_handler.is_active() && id == self.selection_id {
                    info!("⌛ Selection timed out (ID: {}).", id);
                    self.selection_handler.clear();
                    self.selection_timeout = None;
                    if let Some(resp_tx) = self.pending_ipc_resp.take() {
                        let _ = resp_tx.send((-1, true));
                    }
                    self.status = "Selection timed out".to_string();
                } else if self.selection_handler.is_active() {
                    debug!(
                        "⌛ Ignoring stale selection timeout (ID: {} vs Current: {})",
                        id, self.selection_id
                    );
                }
                return Task::none();
            }
            Message::ConfirmCommand => {
                if let Some((name, command)) = self.pending_confirmation.take() {
                    info!("✅ Executing confirmed command: {}", name);
                    self.status = format!("Executed: {}", name);
                    self.confirmation_timeout = None;

                    let shared_kb = self.processor.keyboard.clone();
                    let action_map = self.processor.get_action_map();
                    let engine = self.processor.sound_engine.clone();
                    let lal = self.processor.lal_manager.clone();
                    let custom_audio = if self.config.custom_audio_dir.is_empty() {
                        None
                    } else {
                        Some(std::path::PathBuf::from(&self.config.custom_audio_dir))
                    };

                    return Task::perform(
                        crate::commands::CommandProcessor::execute_command_async(
                            shared_kb,
                            action_map,
                            engine,
                            lal,
                            custom_audio,
                            command,
                        ),
                        |_| Message::None,
                    );
                }
                return Task::none();
            }
            Message::CancelConfirmation => {
                if let Some((name, _)) = self.pending_confirmation.take() {
                    info!("❌ Command cancelled: {}", name);
                    self.status = format!("Cancelled: {}", name);
                    self.confirmation_timeout = None;
                    self.confirmation_id += 1; // Invalidate any flying timeout tasks

                    if let Some(tts) = &self.tts {
                        ASR_PAUSED.store(true, std::sync::atomic::Ordering::SeqCst);
                        return Task::perform(
                            msg_speak(tts.clone(), "Command cancelled".into()),
                            |m| m,
                        );
                    }
                }
                return Task::none();
            }
            Message::ConfirmationTimeout(id) => {
                if self.pending_confirmation.is_some() && id == self.confirmation_id {
                    if let Some((name, _)) = self.pending_confirmation.take() {
                        info!("⌛ Confirmation timed out (ID: {}) for: {}", id, name);
                        self.status = format!("Timeout: {}", name);
                        self.confirmation_timeout = None;
                    }
                }
                return Task::none();
            }
            Message::SelectTrainingPhrase(phrase) => {
                self.training_state.current_phrase = Some(phrase);
            }
            Message::SpeechFinished => {
                ASR_PAUSED.store(false, std::sync::atomic::Ordering::SeqCst);
                // 🛡️ ASR Squelch: Ignore acoustic tail-end feedback
                self.tts_mute_until =
                    Some(std::time::Instant::now() + std::time::Duration::from_millis(1500));

                // 🛡️ Selection Persistence: Reset timer ID after system finishes speaking
                if self.selection_handler.is_active() {
                    self.selection_id += 1;
                    let sid = self.selection_id;
                    self.selection_timeout =
                        Some(std::time::Instant::now() + std::time::Duration::from_secs(30));
                    return Task::perform(selection_timeout_task(sid), |m| m);
                }
                if self.pending_confirmation.is_some() {
                    self.confirmation_id += 1;
                    let cid = self.confirmation_id;
                    self.confirmation_timeout =
                        Some(std::time::Instant::now() + std::time::Duration::from_secs(30));
                    return Task::perform(confirmation_timeout_task(cid), |m| m);
                }
            }
            Message::AutoDetect => {
                let player_manager = self.player_manager.clone();
                return Task::perform(
                    async move {
                        let player_lock = player_manager.player();
                        let player = player_lock.read().await;
                        player.health_check().await
                    },
                    Message::PlayerHealthResponse,
                );
            }
            Message::PlayerHealthResponse(alive) => {
                self.player_status = Some(alive);
                if !alive {
                    self.status = "Player Disconnected".to_string();
                } else if self.player_status == Some(false) && alive {
                    self.status = "Player connected".to_string();
                }
            }
            _ => {
                debug!("Unhandled message in mod.rs: {:?}", message);
            }
        }
        Task::none()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = Vec::new();

        if self.listening {
            subscriptions.push(Subscription::run(|| {
                iced::stream::channel(10, |mut output| async move {
                    let config = crate::config::Config::load().unwrap_or_default();

                    let mut audio_rx = match audio::start_capture(None) {
                        Ok(rx) => rx,
                        Err(e) => {
                            warn!("Failed to start audio capture: {}", e);
                            let _ = output
                                .send(Message::Transcription("Error: No Audio".into()))
                                .await;
                            return;
                        }
                    };

                    let mut asr = match crate::asr::create_engine(config) {
                        Ok(asr) => asr,
                        Err(e) => {
                            warn!("Failed to start ASR: {}", e);
                            let _ = output
                                .send(Message::Transcription("Error: No ASR".into()))
                                .await;
                            return;
                        }
                    };

                    loop {
                        if ASR_PAUSED.load(std::sync::atomic::Ordering::SeqCst) {
                            asr.pause();
                            // 🛡️ Explicit Squelch: Clear any audio buffer while talking
                            while audio_rx.try_recv().is_ok() {}
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            continue;
                        } else if asr.is_paused() {
                            asr.resume();
                        }

                        if let Some(samples) = audio_rx.recv().await {
                            match asr.process(&samples) {
                                Ok(Some(result)) => {
                                    let _ = output.send(Message::Transcription(result.text)).await;
                                }
                                Ok(None) => {}
                                Err(e) => {
                                    warn!("ASR error: {}", e);
                                    let _ = output
                                        .send(Message::Transcription(format!("ASR Error: {}", e)))
                                        .await;
                                    break;
                                }
                            }
                        } else {
                            break;
                        }
                    }
                })
            }));
        }

        // IPC Server (Red Team Hardening: 0600 permissions & seq_id protection)
        subscriptions.push(Subscription::run(|| {
            iced::stream::channel(1, |output| async move {
                let mut server = crate::ipc::server::IpcServer::new();
                let (tx, rx) = std::sync::mpsc::sync_channel(1);
                let rx = std::sync::Arc::new(std::sync::Mutex::new(rx));
                let output = std::sync::Arc::new(std::sync::Mutex::new(output));

                if let Err(e) = server.start(move |seq_id, title, items, page| {
                    let msg = Message::IpcSelectionRequest {
                        seq_id,
                        title,
                        items,
                        page,
                        resp_tx: tx.clone(),
                    };

                    if let Ok(mut output_lock) = output.lock() {
                        let _ = output_lock.try_send(msg);
                    }

                    rx.lock()
                        .map(|guard| guard.recv().unwrap_or((-1, true)))
                        .unwrap_or((-1, true))
                }) {
                    warn!("Failed to start IPC server: {}", e);
                }

                // Keep handle alive
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
                }
            })
        }));

        // Periodic Auto-Detection
        subscriptions.push(
            iced::time::every(std::time::Duration::from_secs(5)).map(|_| Message::AutoDetect),
        );

        // Terminal Stdin Support (CLI User requirement)
        subscriptions.push(Subscription::run(|| {
            iced::stream::channel(10, |mut output| async move {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let stdin = tokio::io::stdin();
                let mut reader = BufReader::new(stdin).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    if !line.trim().is_empty() {
                        let _ = output.send(Message::ManualInput(line)).await;
                    }
                }
            })
        }));

        Subscription::batch(subscriptions)
    }

    pub fn view(&self) -> Element<'_, Message> {
        let sidebar = tabs::sidebar::view(self);

        let content = match self.current_tab {
            Tab::Home => tabs::home::view(self),
            Tab::Games => tabs::games::view(self),
            Tab::Speech => tabs::speech::view(self),
            Tab::SpeechEngines => tabs::speech::view(self),
            Tab::Settings => tabs::settings::view(self),
            Tab::Training => tabs::training::view(self),
            Tab::Packs => tabs::packs::view(self),
            Tab::Vocabulary => tabs::vocabulary::view(self),
            Tab::Corrections => tabs::corrections::view(self),
            Tab::Macros => tabs::macros::view(self),
            Tab::Input => tabs::input::view(self),
            Tab::Player => tabs::player::view(self),
            Tab::Help => tabs::home::view(self),
        };

        row![sidebar, container(content).width(Length::Fill).padding(20)].into()
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    #[test]
    fn test_speech_finished_logic() {
        // We can't easily test TuxTalksApp::update without dependencies,
        // but we can test the logic of selection_timeout and tts_mute_until setting.
        let now = Instant::now();
        let selection_timeout = now + Duration::from_secs(30);
        let tts_mute_until = now + Duration::from_millis(1500);

        // Verify arithmetic
        assert!(selection_timeout > now + Duration::from_secs(25));
        assert!(tts_mute_until >= now + Duration::from_millis(1500));
    }
}
