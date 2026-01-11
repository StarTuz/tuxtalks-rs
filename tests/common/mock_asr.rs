//! Mock ASR Engine for Testing
//!
//! Provides controlled responses for integration tests.

use anyhow::Result;
use std::sync::{Arc, Mutex};
use tuxtalks::asr::{AsrEngine, AsrResult};

/// Mock ASR engine that returns predetermined responses
pub struct MockAsr {
    /// Queue of responses to return
    pub responses: Vec<AsrResult>,
    /// Current index in responses
    idx: usize,
    /// Track if paused
    paused: bool,
    /// Record all audio chunks received (for verification)
    pub received_chunks: Arc<Mutex<Vec<Vec<i16>>>>,
}

impl MockAsr {
    pub fn new(responses: Vec<AsrResult>) -> Self {
        Self {
            responses,
            idx: 0,
            paused: false,
            received_chunks: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Create a mock that returns a single phrase
    pub fn with_phrase(text: &str, confidence: f32) -> Self {
        Self::new(vec![AsrResult {
            text: text.to_string(),
            confidence,
        }])
    }
}

impl AsrEngine for MockAsr {
    fn process(&mut self, samples: &[i16]) -> Result<Option<AsrResult>> {
        // Record received audio
        if let Ok(mut chunks) = self.received_chunks.lock() {
            chunks.push(samples.to_vec());
        }

        // Return nothing if paused
        if self.paused {
            return Ok(None);
        }

        // Return next response if available
        if self.idx < self.responses.len() {
            let result = self.responses[self.idx].clone();
            self.idx += 1;
            Ok(Some(result))
        } else {
            Ok(None)
        }
    }

    fn reset(&mut self) {
        self.idx = 0;
    }

    fn pause(&mut self) {
        self.paused = true;
    }

    fn resume(&mut self) {
        self.paused = false;
    }

    fn is_paused(&self) -> bool {
        self.paused
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_asr_returns_responses() {
        let mut mock = MockAsr::with_phrase("hello world", 0.95);
        let result = mock.process(&[0i16; 100]).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().text, "hello world");
    }

    #[test]
    fn test_mock_asr_paused_returns_nothing() {
        let mut mock = MockAsr::with_phrase("hello", 0.9);
        mock.pause();
        let result = mock.process(&[0i16; 100]).unwrap();
        assert!(result.is_none());
    }
}
