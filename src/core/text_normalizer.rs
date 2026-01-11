//! Text Normalization
//!
//! Handles common ASR errors and text preprocessing before command matching.

use std::collections::HashMap;

/// Normalizes spoken text to fix common recognition errors
pub struct TextNormalizer {
    /// Manual corrections from config
    corrections: HashMap<String, String>,
    /// Number word mappings
    number_words: HashMap<&'static str, u32>,
}

impl TextNormalizer {
    /// Create a new text normalizer
    pub fn new(corrections: HashMap<String, String>) -> Self {
        let mut number_words = HashMap::new();

        // Basic number words
        for (word, num) in [
            ("one", 1),
            ("two", 2),
            ("three", 3),
            ("four", 4),
            ("five", 5),
            ("six", 6),
            ("seven", 7),
            ("eight", 8),
            ("nine", 9),
            ("ten", 10),
            ("eleven", 11),
            ("twelve", 12),
            ("thirteen", 13),
            ("fourteen", 14),
            ("fifteen", 15),
            ("sixteen", 16),
            ("seventeen", 17),
            ("eighteen", 18),
            ("nineteen", 19),
            ("twenty", 20),
        ] {
            number_words.insert(word, num);
        }

        Self {
            corrections,
            number_words,
        }
    }

    /// Normalize text with corrections and common fixes
    pub fn normalize(&self, text: &str) -> String {
        let mut result = text.to_lowercase();

        // Apply manual corrections
        for (from, to) in &self.corrections {
            result = result.replace(&from.to_lowercase(), to);
        }

        // 🛡️ Vosk Mishearing Guardrails (Wendy UX)
        let mishearings = [
            ("the plane ", "play "),
            ("the plane number ", "play number "),
        ];
        for (from, to) in mishearings {
            result = result.replace(from, to);
        }

        let result_trim = result.trim();

        // Strip conversational prefixes and articles recursively
        let prefixes = [
            "yes ",
            "ok ",
            "hey ",
            "um ",
            "uh ",
            "the ",
            "a ",
            "an ",
            "to ",
            "pronounced ",
            "of ",
            "by ",
            "from ",
            "into ",
            "my ",
        ];
        let mut final_text = result_trim.to_string();

        loop {
            let mut changed = false;
            for prefix in prefixes {
                if final_text.starts_with(prefix) {
                    final_text = final_text[prefix.len()..].trim().to_string();
                    changed = true;
                    break;
                }
            }
            if !changed {
                break;
            }
        }

        // 🛡️ Vosk Robustness: Remove redundant articles from middle (Jaan requirement)
        let middle_junk = [" the ", " a ", " an ", " of ", " by ", " my "];
        for junk in middle_junk {
            final_text = final_text.replace(junk, " ");
        }

        // Final cleanup of double spaces
        final_text.replace("  ", " ").trim().to_string()
    }

    /// Parse a spoken number (1-99) from text
    pub fn parse_number(&self, text: &str) -> Option<u32> {
        let text_lower = text.to_lowercase();

        // Try direct number
        if let Ok(num) = text_lower.parse::<u32>() {
            if num > 0 && num <= 99 {
                return Some(num);
            }
        }

        // Try word
        if let Some(&num) = self.number_words.get(text_lower.as_str()) {
            return Some(num);
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_basic() {
        let normalizer = TextNormalizer::new(HashMap::new());
        assert_eq!(normalizer.normalize("HELLO WORLD"), "hello world");
    }

    #[test]
    fn test_normalize_corrections() {
        let mut corrections = HashMap::new();
        corrections.insert("fire physics".to_string(), "fire phasers".to_string());
        let normalizer = TextNormalizer::new(corrections);
        assert_eq!(normalizer.normalize("FIRE PHYSICS"), "fire phasers");
        assert_eq!(normalizer.normalize("ok hey fire physics"), "fire phasers");
    }

    #[test]
    fn test_normalize_articles() {
        let normalizer = TextNormalizer::new(HashMap::new());
        assert_eq!(normalizer.normalize("the play beethoven"), "play beethoven");
        assert_eq!(
            normalizer.normalize("the uh play beethoven"),
            "play beethoven"
        );
        assert_eq!(normalizer.normalize("pronounced beethoven"), "beethoven");
        assert_eq!(normalizer.normalize("the plane of the six"), "play six");
        assert_eq!(
            normalizer.normalize("into my beethoven search"),
            "beethoven search"
        );
    }

    #[test]
    fn test_parse_number() {
        let normalizer = TextNormalizer::new(HashMap::new());
        assert_eq!(normalizer.parse_number("five"), Some(5));
        assert_eq!(normalizer.parse_number("12"), Some(12));
        assert_eq!(normalizer.parse_number("invalid"), None);
    }
}
