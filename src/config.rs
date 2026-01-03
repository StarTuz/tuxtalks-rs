//! Configuration management

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// ASR engine to use
    pub asr_engine: AsrEngine,

    /// Audio input device index (None = default)
    pub audio_device: Option<usize>,

    /// Wake word (if any)
    pub wake_word: Option<String>,

    /// Voice fingerprint path
    pub fingerprint_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AsrEngine {
    Vosk,
    Whisper,
    SpeechdNg,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            asr_engine: AsrEngine::Vosk,
            audio_device: None,
            wake_word: Some("computer".to_string()),
            fingerprint_path: default_fingerprint_path(),
        }
    }
}

impl Config {
    /// Load config from file or create default
    pub fn load() -> Result<Self> {
        let config_path = config_path();

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            Ok(serde_json::from_str(&content)?)
        } else {
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
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tuxtalks")
        .join("config.json")
}

fn default_fingerprint_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tuxtalks")
        .join("fingerprint.json")
}
