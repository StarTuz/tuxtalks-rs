//! TuxTalks Error Types
//!
//! Centralized error handling following Jaana's requirements.

use thiserror::Error;

/// Central error type for TuxTalks
#[derive(Error, Debug)]
pub enum TuxError {
    #[error("ASR engine error: {0}")]
    Asr(String),

    #[error("TTS engine error: {0}")]
    Tts(String),

    #[error("Audio capture error: {0}")]
    Audio(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IPC error: {0}")]
    Ipc(String),

    #[error("Game profile error: {0}")]
    Game(String),

    #[error("Lock poisoned: {0}")]
    Lock(String),

    #[error("Voice fingerprint error: {0}")]
    VoiceFingerprint(String),

    #[error("Player error: {0}")]
    Player(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Result type alias for TuxTalks operations
pub type TuxResult<T> = Result<T, TuxError>;

/// Helper to convert Mutex poison errors
impl<T> From<std::sync::PoisonError<T>> for TuxError {
    fn from(err: std::sync::PoisonError<T>) -> Self {
        TuxError::Lock(err.to_string())
    }
}
