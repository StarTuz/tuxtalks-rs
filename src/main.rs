//! TuxTalks - Open Source AI Media Companion for Linux
//!
//! A Rust implementation of the TuxTalks voice assistant.

use anyhow::Result;
use clap::Parser;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn, Level};
use tracing_subscriber::FmtSubscriber;
use tuxtalks::asr;
use tuxtalks::commands::CommandProcessor;
use tuxtalks::input::{parse_key, InputListener, PttMode};
use tuxtalks::{audio, games, tts};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Audio input device index
    #[arg(short, long)]
    device: Option<usize>,

    /// Use speechd-ng for TTS feedback
    #[arg(long)]
    speechd: bool,

    /// Wake word (overrides config)
    #[arg(short, long)]
    wake_word: Option<String>,
}

#[derive(Debug, PartialEq)]
enum AssistantState {
    Listening,
    CommandMode {
        started_at: Instant,
    },
    SelectionMode {
        started_at: Instant,
        query: String,
        results: Vec<tuxtalks::players::SearchResult>,
    },
    ConfirmationMode {
        started_at: Instant,
        action: String,
        command: tuxtalks::commands::Command,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Setup logging
    let level = if args.verbose {
        Level::DEBUG
    } else {
        Level::INFO
    };
    let subscriber = FmtSubscriber::builder().with_max_level(level).finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("🐧 TuxTalks v{} starting...", env!("CARGO_PKG_VERSION"));

    // Help utility for audit logging (Stamos requirement)
    fn flush_audit_log(entry: &str) -> anyhow::Result<()> {
        let config_dir = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from(".config"));
        let log_dir = config_dir.join("tuxtalks");
        std::fs::create_dir_all(&log_dir)?;

        let log_path = log_dir.join("audit.log");
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)?;

        use std::io::Write;
        writeln!(
            file,
            "[{}] {}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            entry
        )?;
        Ok(())
    }

    // Initialize audio capture
    // Initialize audio capture (Non-fatal for invalid devices/CI)
    let mut audio_rx = match audio::start_capture(args.device) {
        Ok(rx) => {
            info!("🎙️ Audio capture started");
            Some(rx)
        }
        Err(e) => {
            warn!(
                "⚠️ Audio capture failed (running in headless/text-only mode): {}",
                e
            );
            None
        }
    };

    // Initialize command processor
    let mut processor = CommandProcessor::new()?;

    // Initialize game manager
    let mut game_manager = games::GameManager::new()?;

    // Load configuration
    let app_config = tuxtalks::config::Config::load().unwrap_or_default();
    info!(
        "⚙️ Configuration loaded (Wake Word: '{}')",
        app_config.wake_word
    );

    // Initialize ASR
    // Initialize ASR (Non-fatal for headless)
    let mut asr = match asr::create_engine(app_config.clone()) {
        Ok(engine) => Some(engine),
        Err(e) => {
            warn!("⚠️ ASR initialization failed: {}", e);
            None
        }
    };

    // Initialize Shared Services

    // 1. Local Library (needed for player manager)
    let library_path = std::path::PathBuf::from(&app_config.library_db_path);
    let library = Arc::new(tuxtalks::library::LocalLibrary::new(library_path)?);

    // 2. Player Manager
    let player_manager = Arc::new(tuxtalks::player_manager::PlayerManager::new(
        app_config.clone(),
        library,
    ));
    processor.set_player_manager(player_manager.clone());
    info!("🎵 Player Manager initialized");

    // 3. Ollama Handler
    let ollama_handler = tuxtalks::core::ollama::OllamaHandler::new(&app_config);
    if ollama_handler.is_enabled() {
        // Background health check
        let handler_clone = tuxtalks::core::ollama::OllamaHandler::new(&app_config); // Clone by re-creating for now (stateless)
        tokio::spawn(async move {
            if handler_clone.health_check().await {
                info!("🧠 Ollama is ONLINE");
            } else {
                warn!("⚠️ Ollama enabled but UNREACHABLE - will fall back to RegEx");
            }
        });
    }
    processor.set_ollama_handler(ollama_handler);

    if let Some(idx) = game_manager.detect_active_profile() {
        let profile = &game_manager.profiles[idx];
        info!("🎯 Auto-detected active game: {}", profile.name);

        let commands = profile.get_processor_commands();
        for cmd in commands {
            processor.add_command(cmd);
        }
        processor.set_action_map(profile.resolve_actions());
    } else {
        info!("💡 No active game detected, using demo bindings");
        processor.add_demo_bindings();
    }

    // Initialize Sound Engine
    let sound_engine = Arc::new(audio::SoundEngine::new().expect("Failed to init sound engine"));
    processor.set_sound_engine(sound_engine.clone());

    // Initialize TTS
    let tts_engine = tts::create_engine(app_config.clone(), None).await.ok();
    // NOTE (Jony): Do NOT announce on daemon startup.
    // Announcement only happens in GUI when user clicks Start Listening.

    // Initialize Input Listener (PTT & Shortcuts)
    let ptt_key = parse_key(&app_config.ptt_key);

    // Start IPC Server (Background)
    // We clone necessary resources for the server to control the daemon/players
    // Note: To fully implement IPC control, we might need a channel to the main loop or shared state.
    // For now, let's just start it to verify connectivity and audit logs.

    // Create server instance
    let mut ipc_server = tuxtalks::ipc::IpcServer::new();

    // Start with a basic callback for selection requests
    // Format: id, prompt, options, default -> (selected_idx, cancelled)
    let callback = |_id, prompt, options: Vec<String>, _default_idx| -> (i32, bool) {
        info!(
            "📥 IPC Selection Request: '{}' (options: {})",
            prompt,
            options.len()
        );
        // For now, we don't have a way to pop up a GUI from here (unless we were the launcher).
        // The Daemon receives SEARCH requests, but this `start` method seems designed for the LAUNCHER?
        // Wait, `IpcServer` is usually the one *receiving* requests.
        // If this is the DAEMON, it should handle `SearchRequest` and `SelectionResponse`.

        // Actually, looking at `server.rs`, `handle_client` processes messages.
        // If `start` takes a callback, is it for *outgoing* requests? Or incoming?
        // Usually `start` implies listening.
        // If `tuxtalks` daemon is the SERVER, it listens.
        // The callback signature `(u64, String, Vec<String>, usize) -> (i32, bool)` looks like a "get selection from user" function.
        // This implies `IpcServer` code might be shared or mainly for the GUI to receive "Show Selection" requests?

        // If the Daemon runs the server, it receives `SelectionRequest`? No, the Daemon *sends* `SelectionRequest` to the GUI.
        // The GUI runs the Server?
        // Let's check `ipc/messages.rs` to see who sends what.
        (-1, true) // Always cancel for now to avoid blocking
    };

    match ipc_server.start(callback) {
        Ok(_) => info!("🔗 IPC Server started"),
        Err(e) => warn!("⚠️ Failed to start IPC server: {}", e),
    }

    let ptt_mode = match app_config.ptt_mode.to_uppercase().as_str() {
        "TOGGLE" => PttMode::Toggle,
        _ => PttMode::Hold,
    };

    let mut listener = InputListener::new(ptt_key, ptt_mode);

    // Default bindings
    listener.add_binding(evdev::Key::KEY_RIGHT, "next".to_string());
    listener.add_binding(evdev::Key::KEY_LEFT, "previous".to_string());

    let mut listener_rx = listener.start()?;
    info!(
        "👂 Global input listener started (PTT: {:?}, Mode: {:?})",
        ptt_key, ptt_mode
    );

    // Assistant state
    let mut state = AssistantState::Listening;
    let wake_word = args
        .wake_word
        .unwrap_or(app_config.wake_word.clone())
        .to_lowercase();
    let command_timeout = Duration::from_secs(app_config.command_timeout);

    // Main loop
    info!("✅ TuxTalks ready - say '{}' or use PTT", wake_word);
    let mut timeout_check = tokio::time::interval(Duration::from_millis(500));

    loop {
        tokio::select! {
            // Handle audio samples (if active)
            Some(samples) = async {
                match &mut audio_rx {
                    Some(rx) => rx.recv().await,
                    None => std::future::pending().await,
                }
            } => {
                if let Some(engine) = &mut asr {
                    if let Some(result) = engine.process(&samples)? {
                        if result.text.is_empty() { continue; }

                    let normalized = result.text.to_lowercase();
                    debug!("📝 Raw ASR: '{}' (confidence: {:.2})", normalized, result.confidence);

                    // Process based on state or PTT
                    let ptt_active = listener.is_ptt_active();
                    let mut cmd_to_run = None;

                    match state {
                        AssistantState::Listening => {
                            // 1. Guardrail: ASR Confidence Gate (Wendy)
                            if result.confidence < 0.5 {
                                warn!("🔇 Rejecting low-confidence transcription: '{}' ({:.2})", normalized, result.confidence);
                                continue;
                            }

                            // 2. Robust Wake Word Detection (Jaan)
                            // Remove leading punctuation/whitespace
                            let clean_text = normalized.trim_start_matches(|c: char| !c.is_alphanumeric() && !c.is_whitespace()).trim();

                            let mut wake_detected = false;
                            let mut remainder = String::new();

                            if clean_text.starts_with(&wake_word) {
                                wake_detected = true;
                                remainder = clean_text[wake_word.len()..].trim().to_string();
                            } else {
                                // "um... mango" / "uh mango" strategy
                                // Check if wake word is the SECOND token and FIRST token is short (< 4 chars)
                                let parts: Vec<&str> = clean_text.split_whitespace().collect();
                                if parts.len() >= 2 && parts[1] == wake_word && parts[0].len() < 4 {
                                    info!("👂 Soft wake word match (ignoring '{}')", parts[0]);
                                    wake_detected = true;
                                    // Reconstruct remainder
                                    remainder = parts[2..].join(" ");
                                }
                            }

                            if wake_detected {
                                let _ = flush_audit_log(&format!("Wake Word Detected (Confidence: {:.2})", result.confidence)); // Alex

                                if remainder.is_empty() {
                                    info!("🔔 Wake word detected! Entering command mode...");
                                    state = AssistantState::CommandMode { started_at: Instant::now() };
                                    if let Some(ref engine) = tts_engine {
                                        if let Some(ref mut asr_engine) = asr { asr_engine.pause(); }
                                        let _ = engine.speak("Yes?").await; // Jony
                                        if let Some(ref mut asr_engine) = asr { asr_engine.resume(); }
                                    }
                                } else {
                                    info!("🔔 Wake word + Command: '{}'", remainder);
                                    cmd_to_run = Some(remainder);
                                    // Refresh command mode timer
                                    state = AssistantState::CommandMode { started_at: Instant::now() };

                                    // Acknowledge look-forward (Wendy/Jony UX)
                                    if let Some(ref engine) = tts_engine {
                                        if let Some(ref mut asr_engine) = asr { asr_engine.pause(); }
                                        let _ = engine.speak("Acknowledged.").await;
                                        if let Some(ref mut asr_engine) = asr { asr_engine.resume(); }
                                    }
                                }
                            } else if ptt_active {
                                info!("🎤 PTT Command: '{}'", normalized);
                                cmd_to_run = Some(normalized);
                            } else {
                                debug!("🤷 Ignored (no wake word/PTT): {}", normalized);
                            }
                        }
                        AssistantState::CommandMode { .. } => {
                            info!("🎯 Command Mode: '{}'", normalized);
                            cmd_to_run = Some(normalized);
                            // Refresh timer
                            state = AssistantState::CommandMode { started_at: Instant::now() };
                        }
                        AssistantState::SelectionMode { ref results, .. } => {
                            info!("🔢 Selection Mode: '{}'", normalized);

                            let mut selected_idx = None;

                            // Match numbers or keywords
                            if normalized.contains("one") || normalized.contains("first") || normalized == "1" {
                                selected_idx = Some(0);
                            } else if (normalized.contains("two") || normalized.contains("second") || normalized == "2") && results.len() > 1 {
                                selected_idx = Some(1);
                            } else if (normalized.contains("three") || normalized.contains("third") || normalized == "3") && results.len() > 2 {
                                selected_idx = Some(2);
                            } else if normalized.contains("cancel") || normalized.contains("stop") {
                                info!("🚫 Selection cancelled");
                                state = AssistantState::Listening;
                                continue;
                            }

                            if let Some(idx) = selected_idx {
                                info!("✅ Selection made: {}", results[idx].display);
                                let selected = &results[idx];
                                let player_arc = player_manager.player();
                                let player = player_arc.read().await;
                                let _ = match selected.result_type {
                                    tuxtalks::players::SearchResultType::Artist => player.play_artist(&selected.value).await,
                                    tuxtalks::players::SearchResultType::Album => player.play_album(&selected.value).await,
                                    tuxtalks::players::SearchResultType::Song => player.play_song(&selected.value).await,
                                    tuxtalks::players::SearchResultType::Playlist => player.play_playlist(&selected.value, false).await,
                                    tuxtalks::players::SearchResultType::Genre => player.play_genre(&selected.value).await,
                                };
                                state = AssistantState::Listening;
                            } else {
                                warn!("❓ Invalid selection: '{}'. Please say a number 1-{}", normalized, results.len().min(3));
                            }
                            continue; // Skip the general cmd processing
                        }
                        AssistantState::ConfirmationMode { ref action, ref command, .. } => {
                             info!("🛡️ Confirmation Mode: '{}'", normalized);
                             if normalized == "confirm" || normalized == "yes" || normalized == "do it" {
                                 info!("✅ Command confirmed: {}", action);
                                 let _ = flush_audit_log(&format!("Confirmed & Executed: {}", action));
                                 let shared_kb = processor.keyboard.clone();
                                 let action_map = processor.get_action_map();
                                 let sound_engine = processor.sound_engine.clone();
                                 let lal = processor.lal_manager.clone();

                                 tokio::spawn(tuxtalks::commands::CommandProcessor::execute_command_async(
                                     shared_kb,
                                     action_map,
                                     sound_engine,
                                     lal,
                                     None,
                                     command.clone(),
                                 ));
                                 state = AssistantState::Listening;
                             } else if normalized == "cancel" || normalized == "no" || normalized == "abort" {
                                 info!("❌ Command cancelled");
                                 state = AssistantState::Listening;
                             }
                             continue;
                        }
                    }

                    if let Some(cmd_to_run) = cmd_to_run {
                        match processor.process(&cmd_to_run).await {
                            tuxtalks::commands::ProcessResult::Success(cmd) => {
                                info!("✅ Command Executed: {}", cmd);
                                let _ = flush_audit_log(&format!("Executed: {}", cmd));
                            }
                            tuxtalks::commands::ProcessResult::SuccessWithCorrection { action, original, corrected } => {
                                info!("✅ Command Executed: {} (Corrected '{}' -> '{}')", action, original, corrected);
                                let _ = flush_audit_log(&format!("Executed: {} (Correction: '{}' -> '{}')", action, original, corrected));
                            }
                            tuxtalks::commands::ProcessResult::SelectionRequired { query, results } => {
                                info!("🤔 Selection required for '{}'", query);

                                let items: Vec<String> = results.iter().map(|r| r.display.clone()).collect();

                                match tuxtalks::ipc::client::IpcClient::send_selection_request(
                                    &format!("Select for: {}", query),
                                    items,
                                    0,
                                    std::time::Duration::from_secs(30)
                                ) {
                                    Ok(Some((idx, cancelled))) => {
                                        if cancelled {
                                            info!("Selection cancelled");
                                        } else {
                                            let idx_usize = idx as usize;
                                            if idx_usize < results.len() {
                                                let selected = &results[idx_usize];
                                                info!("✅ Selected: {}", selected.display);

                                                let player_arc = player_manager.player();
                                                let player = player_arc.read().await;
                                                let _ = match selected.result_type {
                                                    tuxtalks::players::SearchResultType::Artist => player.play_artist(&selected.value).await,
                                                    tuxtalks::players::SearchResultType::Album => player.play_album(&selected.value).await,
                                                    tuxtalks::players::SearchResultType::Song => player.play_song(&selected.value).await,
                                                    tuxtalks::players::SearchResultType::Playlist => player.play_playlist(&selected.value, false).await,
                                                    tuxtalks::players::SearchResultType::Genre => player.play_genre(&selected.value).await,
                                                };
                                            }
                                        }
                                    }
                                    Err(_) => {
                                        warn!("❌ GUI not reachable via IPC, switching to Voice Selection.");

                                        // Announce options
                                        if let Some(ref engine) = tts_engine {
                                            let mut prompt = format!("Multiple results for {}. ", query);
                                            for (i, res) in results.iter().take(3).enumerate() {
                                                prompt.push_str(&format!("{}: {}. ", i+1, res.display));
                                            }
                                            prompt.push_str("Which one do you want?");
                                            let _ = engine.speak(&prompt).await;
                                        }

                                        state = AssistantState::SelectionMode {
                                            started_at: Instant::now(),
                                            query: query.clone(),
                                            results: results.clone(),
                                        };
                                    }
                                    _ => warn!("IPC selection failed (unexpected response)"),
                                }
                             }
                             tuxtalks::commands::ProcessResult::ConfirmationRequired { action, command } => {
                                 info!("⚠️ Confirmation required for: {}", action);
                                 if let Some(ref engine) = tts_engine {
                                     if let Some(ref mut asr_engine) = asr { asr_engine.pause(); }
                                     let _ = engine.speak(&format!("Dangerous command detected: {}. Say confirm to proceed or cancel to abort.", action)).await;
                                     if let Some(ref mut asr_engine) = asr { asr_engine.resume(); }
                                 }
                                 state = AssistantState::ConfirmationMode {
                                     started_at: Instant::now(),
                                     action,
                                     command,
                                 };
                             }
                             tuxtalks::commands::ProcessResult::NotFound => {
                                warn!("❓ Unknown command: {}", cmd_to_run);
                                if let Some(ref engine) = tts_engine {
                                    if let Some(ref mut asr_engine) = asr { asr_engine.pause(); }
                                    let _ = engine.speak("I didn't catch that command.").await;
                                    if let Some(ref mut asr_engine) = asr { asr_engine.resume(); }
                                }
                            }
                        }
                    }
                }
                }
            }
            // Handle listener commands (shortcuts)
            Some(cmd_text) = listener_rx.recv() => {
                info!("🔑 Shortcut Triggered: {}", cmd_text);
                match processor.process(&cmd_text).await {
                    tuxtalks::commands::ProcessResult::Success(cmd) => {
                         info!("🚀 Executing shortcut: {}", cmd);
                         if let Some(ref engine) = tts_engine {
                             let _ = engine.speak(&format!("Executing {}", cmd)).await;
                         }
                    }
                    tuxtalks::commands::ProcessResult::SuccessWithCorrection { action, .. } => {
                         info!("🚀 Executing shortcut: {} (Ollama corrected)", action);
                         if let Some(ref engine) = tts_engine {
                             let _ = engine.speak(&format!("Executing {}", action)).await;
                         }
                    }
                    tuxtalks::commands::ProcessResult::SelectionRequired { query, results } => {
                        info!("🤔 Shortcut requires selection for '{}'", query);
                        let items: Vec<String> = results.iter().map(|r| r.display.clone()).collect();
                        let _ = tuxtalks::ipc::client::IpcClient::send_selection_request(
                            &format!("Select for: {}", query),
                            items,
                            0,
                            std::time::Duration::from_secs(5)
                        );
                    }
                    _ => {}
                }
            }
            // Periodic timeout check (background)
            _ = timeout_check.tick() => {
                match state {
                    AssistantState::CommandMode { started_at } => {
                        if started_at.elapsed() > command_timeout {
                            info!("⏱ Command mode timed out");
                            state = AssistantState::Listening;
                        }
                    }
                    AssistantState::SelectionMode { started_at, ref query, ref results } => {
                        if started_at.elapsed() > Duration::from_secs(15) {
                            info!("⏱ Selection mode timed out - auto-playing best match for '{}'", query);

                            if let Some(selected) = results.first() {
                                let player_arc = player_manager.player();
                                let player = player_arc.read().await;
                                let _ = match selected.result_type {
                                    tuxtalks::players::SearchResultType::Artist => player.play_artist(&selected.value).await,
                                    tuxtalks::players::SearchResultType::Album => player.play_album(&selected.value).await,
                                    tuxtalks::players::SearchResultType::Song => player.play_song(&selected.value).await,
                                    tuxtalks::players::SearchResultType::Playlist => player.play_playlist(&selected.value, false).await,
                                    tuxtalks::players::SearchResultType::Genre => player.play_genre(&selected.value).await,
                                };
                            }
                            state = AssistantState::Listening;
                        }
                    }
                    AssistantState::ConfirmationMode { started_at, .. } => {
                        if started_at.elapsed() > Duration::from_secs(10) {
                            info!("⏱ Confirmation mode timed out");
                            state = AssistantState::Listening;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
