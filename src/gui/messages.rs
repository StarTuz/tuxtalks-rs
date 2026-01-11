//! Message types for the TuxTalks GUI
//!
//! All messages that can be sent to update the application state.

use crate::gui::wizards;

/// Messages that drive the application
#[derive(Debug, Clone)]
pub enum Message {
    None,
    // Navigation
    TabSelected(super::state::Tab),

    // Core listening
    ToggleListening,
    Transcription(String),
    ManualInput(String),
    StartPressed,
    StopPressed,

    // Profiles
    ProfileSelected(String),
    AutoDetect,
    EditProfile(usize),
    CloseEditor,

    // Profile Editing - Triggers
    AddTrigger(String, String),
    RemoveTrigger(String, String),
    NewTriggerInputChanged(String),

    // Profile Editing - Friendly Names
    AddFriendlyName(String),
    NewFriendlyNameInputChanged(String),
    RemoveFriendlyName(String),

    // Profile Editing - Bindings
    BindingsPathChanged(String),

    // Profile Editing - Macros
    AddMacro(String),
    NewMacroInputChanged(String),
    NewMacroTriggerInputChanged(String),
    NewMacroStepActionChanged(String),
    NewMacroStepDelayChanged(String),
    AddMacroTrigger(String, String, String),
    RemoveMacroTrigger(String, String, String),
    AddMacroStep(String, String, crate::commands::MacroStep),
    RemoveMacroStep(String, String, usize),

    // Speechd / TTS
    SpeechdConnected(std::sync::Arc<dyn crate::tts::TtsEngine>),
    SpeechdFailed,

    // Player Settings
    PlayerBackendChanged(String),
    JRiverIPChanged(String),
    JRiverPortChanged(String),
    JRiverAccessKeyChanged(String),
    StrawberryDbPathChanged(String),
    MprisServiceChanged(String),
    LibraryPathChanged(String),
    ScanLibrary,

    // Input
    PttActive(bool),
    TriggerShortcut(String),
    CommandComplete,

    // App Controls
    SaveConfig,
    LaunchCli,
    LaunchMenu,
    Exit,

    // Wizard
    Wizard(wizards::add_game::WizardMessage),
    SetupWizard(wizards::setup::WizardMessage),
    OpenAddGameWizard,

    // Vocabulary
    AddVocabularyTerm(String),
    RemoveVocabularyTerm(String),
    NewVocabularyInputChanged(String),

    // Corrections
    AddCorrection(String, String),
    RemoveCorrection(String),
    NewCorrectionInChanged(String),
    NewCorrectionOutChanged(String),

    // Training
    ToggleTrainingRecording,
    SelectTrainingPhrase(String),
    ResetFingerprint,
    TrainingRecordingCompleted(Result<std::path::PathBuf, String>),

    // Packs
    InstallPack(String),
    RemovePack(String),

    // Speech Settings
    SelectAsrEngine(String),
    SelectTtsEngine(String),
    WyomingHostChanged(String),
    WyomingPortChanged(String),
    WyomingModelChanged(String),
    WyomingDeviceChanged(String),
    WyomingComputeTypeChanged(String),
    WyomingAutoStartToggled(bool),
    ApplySpeechSettings,

    // General Settings
    WakeWordChanged(String),

    // Ollama Settings
    OllamaEnabledToggled(bool),
    OllamaUrlChanged(String),
    OllamaModelChanged(String),

    // Search
    SearchResults(Vec<crate::players::SearchResult>),
    SelectResult(crate::players::SearchResult, usize),
    ClearSearch,
    OllamaHealthCheck,
    OllamaHealthResponse(bool),
    PlayerHealthCheck,
    PlayerHealthResponse(bool),
    GameCommandResult(crate::commands::ProcessResult),
    SelectVoskModel(String),
    SelectPiperVoice(String),
    ScanModels,
    DeleteVoskModel(String),
    DeletePiperVoice(String),
    BrowseVoskModel,
    BrowsePiperVoice,
    DownloadModel {
        name: String,
        url: String,
        is_voice: bool,
    },
    ModelDownloadProgress(f32),
    ModelDownloadComplete {
        success: bool,
        name: String,
        is_voice: bool,
    },

    // IPC Integration
    IpcSelectionRequest {
        seq_id: u64,
        title: String,
        items: Vec<String>,
        page: usize,
        resp_tx: std::sync::mpsc::SyncSender<(i32, bool)>,
    },

    // Speech Control
    SpeechFinished,

    // Selection Control
    SelectionTimeout(u64),

    // Confirmation Control
    ConfirmCommand,
    CancelConfirmation,
    ConfirmationTimeout(u64),
}
