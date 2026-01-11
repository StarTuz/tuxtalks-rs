//! Piper TTS backend calling a local binary

use super::TtsEngine;
use crate::config::Config;
use anyhow::Result;
use async_trait::async_trait;
use std::io::Write;
use std::process::{Command, Stdio};
use tracing::{debug, error, info, warn};

#[derive(Debug)]
pub struct PiperEngine {
    model_path: String,
    sound_engine: Option<std::sync::Arc<crate::audio::SoundEngine>>,
}

impl PiperEngine {
    pub fn new(config: &Config) -> Result<Self> {
        // Try to find the model path.
        let data_dir = dirs::data_dir().unwrap_or_default().join("tuxtalks/voices");
        let model_filename = format!("{}.onnx", config.piper_voice);
        let model_path = data_dir.join(model_filename);

        if !model_path.exists() {
            warn!("⚠️ Piper model not found at {}", model_path.display());
        }

        Ok(Self {
            model_path: model_path.to_string_lossy().to_string(),
            sound_engine: None, // Will be set or use internal rodio if needed
        })
    }

    pub fn set_sound_engine(&mut self, engine: std::sync::Arc<crate::audio::SoundEngine>) {
        self.sound_engine = Some(engine);
    }
}

#[async_trait]
impl TtsEngine for PiperEngine {
    async fn speak(&self, text: &str) -> Result<()> {
        info!("📢 Piper speaking: '{}'", text);

        if self.model_path.is_empty() || !std::path::Path::new(&self.model_path).exists() {
            return Err(anyhow::anyhow!(
                "Piper model file missing: {}",
                self.model_path
            ));
        }

        // Clone values for move into blocking task
        let model_path = self.model_path.clone();
        let text_owned = text.to_string();
        let sound_engine = self.sound_engine.clone();

        // Move blocking subprocess work to dedicated thread pool
        tokio::task::spawn_blocking(move || -> Result<()> {
            // Generate a temporary WAV file
            let wav_path = std::env::temp_dir().join(format!(
                "tt_speech_{}.wav",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|e| anyhow::anyhow!("Time error: {}", e))?
                    .as_millis()
            ));

            let mut child = Command::new("piper-tts")
                .arg("-m")
                .arg(&model_path)
                .arg("-f")
                .arg(&wav_path)
                .stdin(Stdio::piped())
                .spawn()
                .map_err(|e| {
                    error!("❌ Failed to spawn piper-tts: {}", e);
                    anyhow::anyhow!("Failed to spawn piper-tts: {}", e)
                })?;

            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(text_owned.as_bytes())?;
                stdin.flush()?;
            }

            let status = child.wait()?;
            if !status.success() {
                return Err(anyhow::anyhow!("Piper failed with status {}", status));
            }

            if !wav_path.exists() {
                return Err(anyhow::anyhow!("Piper output file not created"));
            }

            // Play via rodio (SoundEngine if available)
            if let Some(engine) = &sound_engine {
                debug!("✅ Playing Piper WAV via SoundEngine: {:?}", wav_path);
                engine.play_file_sync(&wav_path)?;
            } else {
                debug!("📢 Playing Piper WAV via direct rodio fallback");
                if let Ok((_stream, stream_handle)) = rodio::OutputStream::try_default() {
                    if let Ok(file) = std::fs::File::open(&wav_path) {
                        if let Ok(source) = rodio::Decoder::new(std::io::BufReader::new(file)) {
                            if let Ok(sink) = rodio::Sink::try_new(&stream_handle) {
                                sink.append(source);
                                sink.sleep_until_end();
                            }
                        }
                    }
                }
                let _ = std::fs::remove_file(&wav_path);
            }

            Ok(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("Task join error: {}", e))??;

        Ok(())
    }

    fn name(&self) -> &str {
        "piper"
    }
}
