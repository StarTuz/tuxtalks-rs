//! ASR (Automatic Speech Recognition) module using Vosk

use crate::config::Config;
use anyhow::{Context, Result};
use tracing::{debug, info};
use vosk::{Model, Recognizer};

const SAMPLE_RATE: f32 = 16000.0;

/// Vosk-based ASR engine
pub struct VoskAsr {
    recognizer: Recognizer,
    paused: bool,
}

impl VoskAsr {
    /// Create a new Vosk ASR instance
    pub fn new(config: &Config) -> Result<Self> {
        let model_path = std::path::PathBuf::from(&config.vosk_model_path);

        if !model_path.exists() {
            return Err(anyhow::anyhow!(
                "Vosk model not found at {}",
                model_path.display()
            ));
        }

        info!("Loading Vosk model from: {}", model_path.display());

        let model_str = model_path.to_str().ok_or_else(|| {
            anyhow::anyhow!(
                "Vosk model path is not valid UTF-8: {}",
                model_path.display()
            )
        })?;

        let model = Model::new(model_str).context("Failed to load Vosk model")?;

        let mut grammar = config.custom_vocabulary.clone();
        if !grammar.is_empty() && !config.wake_word.is_empty() {
            let wake_lower = config.wake_word.to_lowercase();
            if !grammar.iter().any(|v| v.to_lowercase() == wake_lower) {
                grammar.push(config.wake_word.clone());
                info!(
                    "📢 Dynamically added wake word '{}' to ASR grammar",
                    config.wake_word
                );
            }
        }

        let recognizer = if grammar.is_empty() {
            Recognizer::new(&model, SAMPLE_RATE).context("Failed to create Vosk recognizer")?
        } else {
            info!("⚙️ Using custom grammar ({} words)", grammar.len());
            Recognizer::new_with_grammar(&model, SAMPLE_RATE, &grammar)
                .context("Failed to create Vosk recognizer with grammar")?
        };

        Ok(Self {
            recognizer,
            paused: false,
        })
    }
}

#[async_trait::async_trait]
impl super::AsrEngine for VoskAsr {
    fn process(&mut self, samples: &[i16]) -> Result<Option<super::AsrResult>> {
        // Discard audio when paused
        if self.paused {
            return Ok(None);
        }

        let state = self.recognizer.accept_waveform(samples);

        match state {
            vosk::DecodingState::Finalized => {
                let result = self.recognizer.final_result();
                if let Some(single) = result.single() {
                    if let Some(text) = extract_text(single.text) {
                        // Calculate average word confidence
                        let avg_confidence = if single.result.is_empty() {
                            1.0f32 // Default if no word-level info
                        } else {
                            let sum: f32 = single.result.iter().map(|w| w.conf).sum();
                            sum / single.result.len() as f32
                        };

                        // Apply confidence filter (Chisholm guardrail)
                        if avg_confidence < super::MIN_CONFIDENCE {
                            info!(
                                "🔇 Rejecting low-confidence ASR ({:.2}): '{}'",
                                avg_confidence, text
                            );
                            return Ok(None);
                        }

                        return Ok(Some(super::AsrResult {
                            text,
                            confidence: avg_confidence,
                        }));
                    }
                }
            }
            vosk::DecodingState::Running => {
                // Partial result - could log for debugging
                debug!("Partial: {}", self.recognizer.partial_result().partial);
            }
            vosk::DecodingState::Failed => {
                debug!("Decoding failed for this chunk");
            }
        }

        Ok(None)
    }

    fn reset(&mut self) {
        self.recognizer.reset();
    }

    fn pause(&mut self) {
        self.paused = true;
        self.recognizer.reset(); // Clear any partial recognition
        debug!("🔇 ASR paused");
    }

    fn resume(&mut self) {
        self.paused = false;
        self.recognizer.reset(); // Also reset on resume to be safe
        debug!("🔊 ASR resumed");
    }

    fn is_paused(&self) -> bool {
        self.paused
    }
}

/// Extract text from Vosk result, filtering empty results
fn extract_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_text() {
        assert_eq!(extract_text(""), None);
        assert_eq!(extract_text("  "), None);
        assert_eq!(extract_text("hello"), Some("hello".to_string()));
        assert_eq!(extract_text("  hello  "), Some("hello".to_string()));
    }
}
