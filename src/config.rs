use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    // Media Player
    pub player: String,
    pub jriver_ip: String,
    pub jriver_port: u16,
    pub access_key: String,
    pub strawberry_db_path: String,
    pub mpris_service: String,
    pub library_path: String,
    pub library_db_path: String,

    // Speech
    pub asr_engine: String,
    pub tts_engine: String,
    pub wake_word: String,
    pub vosk_model_path: String,
    pub piper_voice: String,
    pub command_timeout: u64,

    // Input/PTT
    pub ptt_enabled: bool,
    pub ptt_mode: String,
    pub ptt_key: String,

    // Wyoming
    pub wyoming_host: String,
    pub wyoming_port: u16,
    pub wyoming_auto_start: bool,
    pub wyoming_model: String,
    pub wyoming_device: String,
    pub wyoming_compute_type: String,

    // AI
    pub ollama_enabled: bool,
    pub ollama_url: String,
    pub ollama_model: String,

    // Meta
    pub ui_language: String,
    pub log_level: String,
    pub first_run_complete: bool,
    pub gui_scaling: f64,

    // Data
    pub voice_corrections: HashMap<String, String>,
    pub custom_vocabulary: Vec<String>,

    // Audio
    #[serde(default)]
    pub custom_audio_dir: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            player: "jriver".to_string(),
            jriver_ip: "localhost".to_string(),
            jriver_port: 52199,
            access_key: "".to_string(),
            strawberry_db_path: dirs::data_dir()
                .unwrap_or_default()
                .join("strawberry/strawberry/strawberry.db")
                .to_string_lossy()
                .to_string(),
            mpris_service: "org.mpris.MediaPlayer2.vlc".to_string(),
            library_path: dirs::audio_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            library_db_path: dirs::data_dir()
                .unwrap_or_default()
                .join("tuxtalks-rs/library.db")
                .to_string_lossy()
                .to_string(),
            asr_engine: "vosk".to_string(),
            tts_engine: "piper".to_string(),
            wake_word: "tuxtalks".to_string(),
            vosk_model_path: dirs::data_dir()
                .unwrap_or_default()
                .join("tuxtalks/models/vosk-model-en-gb-small")
                .to_string_lossy()
                .to_string(),
            piper_voice: "en_GB-cori-high".to_string(),
            command_timeout: 5,
            ptt_enabled: false,
            ptt_mode: "HOLD".to_string(),
            ptt_key: "KEY_LEFTCTRL".to_string(),
            wyoming_host: "localhost".to_string(),
            wyoming_port: 10301,
            wyoming_auto_start: true,
            wyoming_model: "tiny".to_string(),
            wyoming_device: "cpu".to_string(),
            wyoming_compute_type: "int8".to_string(),
            ollama_enabled: false,
            ollama_url: "http://localhost:11434".to_string(),
            ollama_model: "llama2".to_string(),
            ui_language: "en".to_string(),
            log_level: "INFO".to_string(),
            first_run_complete: false,
            gui_scaling: 1.0,

            voice_corrections: HashMap::from([
                ("play er".to_string(), "player".to_string()),
                ("too".to_string(), "to".to_string()),
                ("4".to_string(), "four".to_string()),
                ("2".to_string(), "two".to_string()),
            ]),
            custom_vocabulary: Vec::new(),
            custom_audio_dir: dirs::data_dir()
                .unwrap_or_default()
                .join("tuxtalks/audio")
                .to_string_lossy()
                .to_string(),
        }
    }
}

impl Config {
    /// Load config from file, migrate from Python, or create default
    pub fn load() -> Result<Self> {
        let config_path = config_path();

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            match serde_json::from_str(&content) {
                Ok(config) => Ok(config),
                Err(e) => {
                    // Graceful degradation: log warning and use defaults
                    tracing::warn!("⚠️ Config file corrupted or invalid, using defaults: {}", e);
                    // Backup corrupt file for debugging
                    let backup_path = config_path.with_extension("json.corrupt");
                    let _ = std::fs::rename(&config_path, &backup_path);
                    Ok(Self::default())
                }
            }
        } else {
            // Attempt migration from Python
            if let Some(migrated) = Self::migrate_from_python() {
                let _ = migrated.save();
                return Ok(migrated);
            }
            Ok(Self::default())
        }
    }

    /// Save config to file
    pub fn save(&self) -> Result<()> {
        let config_path = config_path();

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }

    fn migrate_from_python() -> Option<Self> {
        let python_config = dirs::config_dir()?.join("tuxtalks").join("config.json");
        if python_config.exists() {
            if let Ok(content) = std::fs::read_to_string(python_config) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                    let mut cfg = Self::default();

                    // Direct mappings
                    if let Some(v) = val.get("WAKE_WORD").and_then(|v| v.as_str()) {
                        cfg.wake_word = v.to_string();
                    }
                    if let Some(v) = val.get("PLAYER").and_then(|v| v.as_str()) {
                        cfg.player = v.to_string();
                    }
                    if let Some(v) = val.get("JRIVER_IP").and_then(|v| v.as_str()) {
                        cfg.jriver_ip = v.to_string();
                    }
                    if let Some(v) = val.get("ACCESS_KEY").and_then(|v| v.as_str()) {
                        cfg.access_key = v.to_string();
                    }
                    if let Some(v) = val.get("ASR_ENGINE").and_then(|v| v.as_str()) {
                        cfg.asr_engine = v.to_string();
                    }
                    if let Some(v) = val.get("TTS_ENGINE").and_then(|v| v.as_str()) {
                        cfg.tts_engine = v.to_string();
                    }
                    if let Some(v) = val.get("PTT_ENABLED").and_then(|v| v.as_bool()) {
                        cfg.ptt_enabled = v;
                    }

                    return Some(cfg);
                }
            }
        }
        None
    }
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tuxtalks-rs")
        .join("config.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.player, "jriver");
        assert_eq!(config.jriver_port, 52199);
        assert_eq!(config.wake_word, "tuxtalks");
        assert_eq!(config.command_timeout, 5);
        assert!(!config.ptt_enabled);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let json = serde_json::to_string(&config).expect("Failed to serialize");
        let restored: Config = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(config.player, restored.player);
        assert_eq!(config.wake_word, restored.wake_word);
    }

    #[test]
    fn test_config_corrupt_json_handling() {
        // Config::load uses graceful degradation - this tests the parsing path
        let corrupt_json = "{ not valid json";
        let result: Result<Config, _> = serde_json::from_str(corrupt_json);
        assert!(result.is_err());
    }
}
