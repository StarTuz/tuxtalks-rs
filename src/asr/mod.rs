//! ASR (Automatic Speech Recognition) Module
//!
//! Provides multiple ASR backends:
//! - Vosk: Local offline recognition
//! - Wyoming: Remote ASR protocol (e.g., faster-whisper)

pub mod vosk;
pub mod wyoming;
pub mod wyoming_manager;

use crate::config::Config;
use anyhow::Result;
use async_trait::async_trait;

// Re-export main types
pub use vosk::VoskAsr;
pub use wyoming::WyomingClient;

/// Result from ASR with confidence score (Chisholm guardrail)
#[derive(Debug, Clone)]
pub struct AsrResult {
    pub text: String,
    pub confidence: f32,
}

/// Minimum confidence threshold (below this, results are discarded)
pub const MIN_CONFIDENCE: f32 = 0.5;

/// Trait for ASR engines
#[async_trait]
pub trait AsrEngine: Send + Sync {
    /// Process audio samples and return recognized text with confidence (if final)
    /// Results below MIN_CONFIDENCE should be filtered out internally
    fn process(&mut self, samples: &[i16]) -> Result<Option<AsrResult>>;

    /// Reset the recognizer state
    fn reset(&mut self);

    /// Pause recognition (discard incoming audio)
    /// Default implementation does nothing (backwards compatible)
    fn pause(&mut self) {}

    /// Resume recognition after pause
    /// Default implementation does nothing (backwards compatible)
    fn resume(&mut self) {}

    /// Check if currently paused
    fn is_paused(&self) -> bool {
        false
    }
}

/// Factory to create the configured ASR engine
pub fn create_engine(config: Config) -> Result<Box<dyn AsrEngine>> {
    match config.asr_engine.as_str() {
        "vosk" => Ok(Box::new(vosk::VoskAsr::new(&config)?)),
        "wyoming" => Ok(Box::new(wyoming::WyomingClient::new(
            &config.wyoming_host,
            config.wyoming_port,
        ))),
        _ => Ok(Box::new(vosk::VoskAsr::new(&config)?)),
    }
}
