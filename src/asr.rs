//! ASR (Automatic Speech Recognition) module using Vosk

use anyhow::{Context, Result};
use std::path::PathBuf;
use tracing::{debug, info};
use vosk::{Model, Recognizer};

const SAMPLE_RATE: f32 = 16000.0;

/// Vosk-based ASR engine
pub struct VoskAsr {
    recognizer: Recognizer,
}

impl VoskAsr {
    /// Create a new Vosk ASR instance
    pub fn new() -> Result<Self> {
        // Find model path
        let model_path = find_model_path()
            .context("Could not find Vosk model. Install with: vosk-model-small-en-us")?;

        info!("Loading Vosk model from: {}", model_path.display());

        let model =
            Model::new(model_path.to_str().unwrap()).context("Failed to load Vosk model")?;

        let recognizer =
            Recognizer::new(&model, SAMPLE_RATE).context("Failed to create Vosk recognizer")?;

        Ok(Self { recognizer })
    }

    /// Process audio samples and return recognized text (if final)
    pub fn process(&mut self, samples: &[i16]) -> Result<Option<String>> {
        let state = self.recognizer.accept_waveform(samples);

        match state {
            vosk::DecodingState::Finalized => {
                let result = self.recognizer.final_result();
                if let Some(single) = result.single() {
                    if let Some(text) = extract_text(&single.text) {
                        return Ok(Some(text));
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

    /// Reset the recognizer state
    pub fn reset(&mut self) {
        self.recognizer.reset();
    }
}

/// Find Vosk model in standard locations
fn find_model_path() -> Option<PathBuf> {
    let candidates: Vec<Option<PathBuf>> = vec![
        // User data dir
        dirs::data_dir().map(|d| d.join("vosk/model")),
        // System-wide
        Some(PathBuf::from("/usr/share/vosk/model")),
        // Common install locations
        Some(PathBuf::from("/usr/share/vosk-model-small-en-us")),
        Some(PathBuf::from("/usr/local/share/vosk/model")),
        // Home directory
        dirs::home_dir().map(|d| d.join(".vosk/model")),
        dirs::home_dir().map(|d| d.join("vosk-model-small-en-us")),
    ];

    for candidate in candidates.into_iter().flatten() {
        if candidate.exists() {
            return Some(candidate);
        }
    }

    None
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
