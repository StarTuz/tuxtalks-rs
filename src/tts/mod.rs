//! TTS (Text-to-Speech) Module
//!
//! Provides a unified interface for multiple TTS backends.

use crate::config::Config;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{info, warn};

pub mod piper;
pub mod speechd;
pub mod system;

/// Trait for TTS engines
#[async_trait]
pub trait TtsEngine: Send + Sync + std::fmt::Debug {
    /// Speak the given text
    async fn speak(&self, text: &str) -> Result<()>;

    /// Get the engine name
    fn name(&self) -> &str;
}

/// Factory to create the configured TTS engine
pub async fn create_engine(
    config: Config,
    sound_engine: Option<Arc<crate::audio::SoundEngine>>,
) -> Result<Arc<dyn TtsEngine>> {
    info!("🛠️ Creating TTS engine: {}", config.tts_engine);
    let engine: Arc<dyn TtsEngine> = match config.tts_engine.as_str() {
        "piper" => {
            info!("  - Using Piper TTS (Voice: {})", config.piper_voice);
            let mut p = piper::PiperEngine::new(&config)?;
            if let Some(se) = sound_engine {
                p.set_sound_engine(se);
            }
            Arc::new(p)
        }
        "speechd_ng" | "speechd" => {
            info!("  - Using Speechd TTS");
            let client = speechd::SpeechdEngine::connect().await?;
            Arc::new(client)
        }
        "system" => {
            info!("  - Using System TTS Fallback");
            Arc::new(system::SystemEngine::new())
        }
        _ => {
            warn!(
                "  - Unknown engine '{}', falling back to System",
                config.tts_engine
            );
            Arc::new(system::SystemEngine::new())
        }
    };
    info!("✅ TTS engine '{}' initialized", engine.name());
    Ok(engine)
}
