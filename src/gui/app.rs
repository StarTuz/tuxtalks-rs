//! Main application state for TuxTalks GUI
//!
//! Contains the TuxTalksApp struct and initialization logic.

use iced::Task;
use std::io::Write;
use std::sync::Arc;
use tracing::info;

use crate::commands::CommandProcessor;
use crate::config::Config;
use crate::core::ollama::OllamaHandler;
use crate::core::text_normalizer::TextNormalizer;
use crate::games::GameManager;
use crate::lal::LALManager;
use crate::library::LocalLibrary;
use crate::voice_fingerprint::VoiceFingerprint;

use super::messages::Message;
use super::state::{SpeechState, Tab, TrainingState};
use super::wizards;
use crate::selection::SelectionHandler;

/// Main application state
pub struct TuxTalksApp {
    /// Current view/tab
    pub(crate) current_tab: Tab,
    /// Status message
    pub(crate) status: String,
    /// Is listening active
    pub(crate) listening: bool,
    /// Recent transcriptions
    pub(crate) transcriptions: Vec<String>,
    /// Game Manager
    pub(crate) game_manager: GameManager,
    /// Command Processor
    pub(crate) processor: CommandProcessor,
    /// TTS Client
    pub(crate) tts: Option<Arc<dyn crate::tts::TtsEngine>>,
    /// Ollama Handler for AI intent extraction
    pub(crate) ollama: OllamaHandler,
    /// Player Manager for media control
    pub(crate) player_manager: Arc<crate::player_manager::PlayerManager>,
    /// Text Normalizer for ASR correction
    pub(crate) normalizer: TextNormalizer,
    /// Index of profile being edited
    pub(crate) editing_profile_idx: Option<usize>,
    /// Input for adding new triggers
    pub(crate) new_trigger_input: String,
    /// Input for adding new command friendly names
    pub(crate) new_friendly_name_input: String,
    /// Input for adding new macro names
    pub(crate) new_macro_input: String,
    /// Input for adding new macro triggers
    pub(crate) new_macro_trigger_input: String,
    /// Input for adding new macro step actions
    pub(crate) new_macro_step_action: String,
    /// Input for adding new macro step delays
    pub(crate) new_macro_step_delay: String,
    /// Input for new vocabulary words
    pub(crate) new_vocab_input: String,
    /// Input for corrections (original)
    pub(crate) new_correction_in: String,
    /// Input for corrections (replacement)
    pub(crate) new_correction_out: String,
    /// Training Tab State
    pub(crate) training_state: TrainingState,
    /// Speech Tab State
    pub(crate) speech_state: SpeechState,
    /// Voice Fingerprint (Backend)
    pub(crate) voice_fingerprint: Arc<VoiceFingerprint>,
    /// Configuration
    pub(crate) config: Config,
    /// Local Library
    pub(crate) _library: Arc<LocalLibrary>,
    /// PTT active status
    pub(crate) ptt_active: bool,
    /// Wizard State
    pub(crate) _add_game_wizard: Option<wizards::add_game::AddGameWizard>,
    /// Ollama health status (None = not checked, Some(true) = healthy, Some(false) = failed)
    pub(crate) ollama_status: Option<bool>,
    /// Player health status (None = not checked, Some(true) = healthy, Some(false) = failed)
    pub(crate) player_status: Option<bool>,
    /// Active command window duration (for wake word persistence)
    pub(crate) _active_command_until: Option<std::time::Instant>,
    /// Sound Engine for SFX
    pub(crate) sound_engine: Arc<crate::audio::SoundEngine>,
    /// Selection Handler for disambiguation
    pub(crate) selection_handler: SelectionHandler,
    /// LAL Manager for content packs
    pub(crate) lal_manager: Arc<LALManager>,
    /// Setup Wizard
    pub(crate) _setup_wizard: Option<wizards::setup::SetupWizard>,
    /// Search Results
    pub(crate) search_results: Vec<crate::players::SearchResult>,
    /// Is searching running
    pub(crate) _searching: bool,
    /// Timestamp until which transcriptions should be ignored (mute window during TTS)
    pub(crate) tts_mute_until: Option<std::time::Instant>,
    /// Last command execution time (rate limiting - Stamos guardrail)
    pub(crate) last_command_time: Option<std::time::Instant>,
    /// Command audit log (last 100 commands - Stamos guardrail)
    pub(crate) command_audit_log: Vec<String>,
    /// Pending IPC response sender
    pub(crate) pending_ipc_resp: Option<std::sync::mpsc::SyncSender<(i32, bool)>>,
    /// Selection timeout (10s)
    pub(crate) selection_timeout: Option<std::time::Instant>,
    /// Pending high-risk command confirmation
    pub(crate) pending_confirmation: Option<(String, crate::commands::Command)>,
    /// Confirmation timeout
    pub(crate) confirmation_timeout: Option<std::time::Instant>,
    /// Command mode state (Jony - wake word triggers this)
    pub(crate) command_mode: CommandModeState,
    /// Command mode timeout (10 seconds per Wendy)
    pub(crate) command_mode_timeout: Option<std::time::Instant>,
    /// Selection ID to track stale timeout tasks
    pub(crate) selection_id: u64,
    /// Confirmation ID to track stale timeout tasks
    pub(crate) confirmation_id: u64,
}

/// Command mode state (Jaana - state machine)
#[derive(Debug, Clone, PartialEq, Default)]
pub enum CommandModeState {
    #[default]
    Inactive,
    Active,
}

impl TuxTalksApp {
    /// Create a new TuxTalksApp instance
    pub fn new() -> (Self, Task<Message>) {
        let game_manager = GameManager::new().expect("Failed to init GameManager");
        let mut processor = CommandProcessor::new().expect("Failed to init CommandProcessor");
        let config = crate::config::Config::load().unwrap_or_default();
        let voice_fingerprint =
            Arc::new(VoiceFingerprint::new().expect("Failed to init VoiceFingerprint"));

        // Initialize Ollama
        let ollama = OllamaHandler::new(&config);

        info!("🚀 TuxTalks app initialized");
        if ollama.is_enabled() {
            info!("🤖 Ollama AI routing enabled ({})", config.ollama_model);
        }

        // Initialize library
        let library = Arc::new(
            LocalLibrary::new(std::path::PathBuf::from(&config.library_db_path))
                .expect("Failed to init LocalLibrary"),
        );

        let sound_engine =
            Arc::new(crate::audio::SoundEngine::new().expect("Failed to init SoundEngine"));
        processor.set_sound_engine(sound_engine.clone());

        // Initialize LAL Manager
        let lal_manager = Arc::new(LALManager::new());

        let player_manager = Arc::new(crate::player_manager::PlayerManager::new(
            config.clone(),
            library.clone(),
        ));
        processor.set_player_manager(player_manager.clone());
        processor.set_ollama_handler(ollama.clone());
        processor.set_lal_manager(lal_manager.clone());

        let mut app = Self {
            current_tab: Tab::Home,
            status: "Ready".to_string(),
            listening: false,
            transcriptions: Vec::new(),
            game_manager,
            processor,
            tts: None, // Will be set by init_task if async, or set here
            ollama,
            player_manager,
            normalizer: TextNormalizer::new(config.voice_corrections.clone()),
            editing_profile_idx: None,
            new_trigger_input: String::new(),
            new_friendly_name_input: String::new(),
            new_macro_input: String::new(),
            new_macro_trigger_input: String::new(),
            new_macro_step_action: String::new(),
            new_macro_step_delay: "100".to_string(),
            new_vocab_input: String::new(),
            new_correction_in: String::new(),
            new_correction_out: String::new(),
            training_state: TrainingState::default(),
            speech_state: SpeechState::default(),
            voice_fingerprint,
            config: config.clone(),
            _library: library,
            ptt_active: false,
            _add_game_wizard: None,
            ollama_status: None,
            player_status: None,
            _active_command_until: None,
            sound_engine: sound_engine.clone(),
            selection_handler: SelectionHandler::new(),
            lal_manager,
            _setup_wizard: if !config.first_run_complete {
                Some(wizards::setup::SetupWizard::new())
            } else {
                None
            },
            search_results: Vec::new(),
            _searching: false,
            tts_mute_until: None,
            last_command_time: None,
            command_audit_log: Vec::new(),
            pending_ipc_resp: None,
            selection_timeout: None,
            pending_confirmation: None,
            confirmation_timeout: None,
            command_mode: CommandModeState::default(),
            command_mode_timeout: None,
            selection_id: 0,
            confirmation_id: 0,
        };

        app.scan_available_models();

        // Initialize TTS in background based on config
        let init_task = Task::perform(
            crate::tts::create_engine(config.clone(), Some(sound_engine.clone())),
            |res| match res {
                Ok(engine) => Message::SpeechdConnected(engine), // Re-using message for now or refactor
                Err(_) => Message::SpeechdFailed,
            },
        );

        (app, init_task)
    }

    pub fn scan_available_models(&mut self) {
        let models_dir = dirs::data_dir().unwrap_or_default().join("tuxtalks/models");

        if !models_dir.exists() {
            return;
        }

        // Scan for Vosk models (directories starting with vosk-model)
        if let Ok(entries) = std::fs::read_dir(&models_dir) {
            self.speech_state.available_vosk_models = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|name| name.contains("vosk-model"))
                .collect();
            self.speech_state.available_vosk_models.sort();
        }

        // Scan for Piper voices (.onnx files in 'voices' dir)
        let voices_dir = dirs::data_dir().unwrap_or_default().join("tuxtalks/voices");

        if let Ok(entries) = std::fs::read_dir(&voices_dir) {
            self.speech_state.available_piper_voices = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
                .filter(|e| e.path().extension().map(|s| s == "onnx").unwrap_or(false))
                .map(|e| {
                    e.path()
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default()
                })
                .filter(|s| !s.is_empty())
                .collect();
            self.speech_state.available_piper_voices.sort();
        }

        info!(
            "🔍 Scanned models: {} Vosk, {} Piper",
            self.speech_state.available_vosk_models.len(),
            self.speech_state.available_piper_voices.len()
        );
    }

    /// Application title
    pub fn title(&self) -> String {
        "TuxTalks - AI Media Companion".to_string()
    }

    /// Application theme
    pub fn theme(&self) -> iced::Theme {
        iced::Theme::Dark
    }

    /// Append an entry to the audit log on disk (Red Team requirement)
    pub fn flush_audit_log(&self, entry: &str) -> anyhow::Result<()> {
        let config_dir = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from(".config"));
        let log_dir = config_dir.join("tuxtalks");
        std::fs::create_dir_all(&log_dir)?;

        let log_path = log_dir.join("audit.log");
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)?;

        writeln!(
            file,
            "[{}] {}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            entry
        )?;

        Ok(())
    }
}

/// Async task to download a model or voice (Jaana Dogan: Hardened Async)
pub async fn download_model_task(
    name: String,
    url: String,
    dest_dir: std::path::PathBuf,
    is_voice: bool,
) -> anyhow::Result<()> {
    tokio::fs::create_dir_all(&dest_dir).await?;

    if is_voice {
        // Piper Voice: Download .onnx and .onnx.json
        let base_url = if url.ends_with(".onnx") {
            &url[..url.len() - 5]
        } else {
            &url
        };

        let onnx_url = format!("{}.onnx", base_url);
        let json_url = format!("{}.onnx.json", base_url);

        let onnx_path = dest_dir.join(format!("{}.onnx", name));
        let json_path = dest_dir.join(format!("{}.onnx.json", name));

        info!("⬇️ Downloading Piper .onnx: {}", onnx_url);
        let resp = reqwest::get(onnx_url).await?;
        let bytes = resp.bytes().await?;
        tokio::fs::write(&onnx_path, bytes).await?;

        info!("⬇️ Downloading Piper .json: {}", json_url);
        let resp = reqwest::get(json_url).await?;
        let bytes = resp.bytes().await?;
        tokio::fs::write(&json_path, bytes).await?;
    } else {
        // Vosk Model: Download zip and extract
        let temp_zip = std::env::temp_dir().join("tt_model_download.zip");

        info!("⬇️ Downloading Vosk zip: {}", url);
        let resp = reqwest::get(url).await?;
        let bytes = resp.bytes().await?;
        tokio::fs::write(&temp_zip, bytes).await?;

        info!("📦 Extracting Vosk model to: {:?}", dest_dir);

        // Use tokio::process for non-blocking unzip
        let status = tokio::process::Command::new("unzip")
            .arg("-o")
            .arg(&temp_zip)
            .arg("-d")
            .arg(&dest_dir)
            .status()
            .await?;

        if !status.success() {
            return Err(anyhow::anyhow!("Unzip failed with status {}", status));
        }

        // Clean up zip
        let _ = tokio::fs::remove_file(&temp_zip).await;
    }

    info!("✅ Download complete for {}", name);
    Ok(())
}
