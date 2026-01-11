//! Mock TTS Engine for Testing
//!
//! Records all spoken text for verification.

use anyhow::Result;
use async_trait::async_trait;
use std::sync::{Arc, Mutex};

/// Mock TTS engine that records spoken text
#[derive(Debug)]
pub struct MockTts {
    /// All text that was "spoken"
    pub spoken: Arc<Mutex<Vec<String>>>,
    /// Simulate failure on next speak
    pub should_fail: Arc<Mutex<bool>>,
}

impl MockTts {
    pub fn new() -> Self {
        Self {
            spoken: Arc::new(Mutex::new(Vec::new())),
            should_fail: Arc::new(Mutex::new(false)),
        }
    }

    /// Get all spoken phrases
    pub fn get_spoken(&self) -> Vec<String> {
        self.spoken.lock().unwrap().clone()
    }

    /// Check if a phrase was spoken
    pub fn was_spoken(&self, text: &str) -> bool {
        self.spoken.lock().unwrap().iter().any(|s| s.contains(text))
    }
}

impl Default for MockTts {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl tuxtalks::tts::TtsEngine for MockTts {
    async fn speak(&self, text: &str) -> Result<()> {
        if *self.should_fail.lock().unwrap() {
            return Err(anyhow::anyhow!("Mock TTS failure"));
        }
        self.spoken.lock().unwrap().push(text.to_string());
        Ok(())
    }

    fn name(&self) -> &str {
        "mock"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_tts_records_speech() {
        use tuxtalks::tts::TtsEngine;

        let mock = MockTts::new();
        mock.speak("hello").await.unwrap();
        mock.speak("world").await.unwrap();

        assert!(mock.was_spoken("hello"));
        assert!(mock.was_spoken("world"));
        assert_eq!(mock.get_spoken().len(), 2);
    }
}
