//! State types for the TuxTalks GUI
//!
//! Enums and structs for application state management.

/// Current tab/view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Home,
    Games,
    Player,
    Speech,
    SpeechEngines,
    Input,
    Vocabulary,
    Corrections,
    Training,
    Packs,
    Settings,
    Macros,
    Help,
}

/// Speech engine configuration state
#[derive(Debug, Clone)]
pub struct SpeechState {
    pub selected_asr: String,
    pub selected_tts: String,
    pub wyoming_host_input: String,
    pub wyoming_port_input: String,
    pub wyoming_model_input: String,
    pub wyoming_device_input: String,
    pub wyoming_compute_type_input: String,
    pub wyoming_auto_start: bool,
    pub available_vosk_models: Vec<String>,
    pub available_piper_voices: Vec<String>,
}

impl Default for SpeechState {
    fn default() -> Self {
        Self {
            selected_asr: "vosk".to_string(),
            selected_tts: "piper".to_string(),
            wyoming_host_input: "localhost".to_string(),
            wyoming_port_input: "10301".to_string(),
            wyoming_model_input: "tiny".to_string(),
            wyoming_device_input: "cpu".to_string(),
            wyoming_compute_type_input: "int8".to_string(),
            wyoming_auto_start: true,
            available_vosk_models: Vec::new(),
            available_piper_voices: Vec::new(),
        }
    }
}

/// Voice training state
#[derive(Debug, Clone, Default)]
pub struct TrainingState {
    pub is_recording: bool,
    pub current_phrase: Option<String>,
    pub progress: f32,
}
