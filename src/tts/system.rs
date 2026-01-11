//! System fallback TTS engine

use super::TtsEngine;
use anyhow::Result;
use async_trait::async_trait;
use std::process::Command;
use tracing::debug;

#[derive(Debug)]
pub struct SystemEngine;

impl Default for SystemEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemEngine {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TtsEngine for SystemEngine {
    async fn speak(&self, text: &str) -> Result<()> {
        debug!("System speaking: {}", text);

        // Try spd-say (speech-dispatcher) or espeak-ng
        if Command::new("spd-say").arg(text).spawn().is_ok() {
            return Ok(());
        }

        if Command::new("espeak-ng").arg(text).spawn().is_ok() {
            return Ok(());
        }

        Err(anyhow::anyhow!(
            "No system TTS command found (tried spd-say, espeak-ng)"
        ))
    }

    fn name(&self) -> &str {
        "system"
    }
}
