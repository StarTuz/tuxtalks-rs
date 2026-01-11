use anyhow::{Context, Result};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use tracing::info;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VoicePattern {
    pub likely_meant: Vec<String>,
    pub confidence: f32,
    pub count: u32,
    pub source: String,    // "passive" or "manual"
    pub last_seen: String, // ISO timestamp
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct FingerprintData {
    pub asr_patterns: HashMap<String, VoicePattern>,
    pub command_frequency: HashMap<String, u32>,
    pub metadata: HashMap<String, String>,
}

pub struct VoiceFingerprint {
    base_path: PathBuf,
    fingerprint_file: PathBuf,
    data: Mutex<FingerprintData>,
}

impl VoiceFingerprint {
    pub fn new() -> Result<Self> {
        let mut base_path = dirs::data_local_dir().context("No data dir")?;
        base_path.push("tuxtalks");

        fs::create_dir_all(&base_path)?;

        let mut training_path = base_path.clone();
        training_path.push("training");
        fs::create_dir_all(&training_path)?;

        let fingerprint_file = base_path.join("voice_fingerprint.json");

        let mut vf = Self {
            base_path: training_path,
            fingerprint_file,
            data: Mutex::new(FingerprintData::default()),
        };

        vf.load().ok(); // Try to load existing data

        Ok(vf)
    }

    /// Learn from successful Ollama correction (PASSIVE LEARNING)
    pub fn add_passive_correction(&self, asr_heard: &str, ollama_resolved: &str) -> bool {
        let (error_word, correct_word) = self.extract_error_pair(asr_heard, ollama_resolved);

        if error_word.is_empty() || correct_word.is_empty() {
            return false;
        }

        let mut data = self.data.lock().expect("VoiceFingerprint mutex poisoned");
        let pattern = data
            .asr_patterns
            .entry(error_word.clone())
            .or_insert_with(|| VoicePattern {
                likely_meant: Vec::new(),
                confidence: 0.0,
                count: 0,
                source: "passive".to_string(),
                last_seen: Local::now().to_rfc3339(),
            });

        pattern.likely_meant.push(correct_word.clone());
        pattern.count += 1;
        pattern.last_seen = Local::now().to_rfc3339();

        self.recalculate_confidence(&error_word, pattern);

        drop(data);
        self.save().ok();

        info!("📚 Learned passive: '{}' -> '{}'", error_word, correct_word);
        true
    }

    /// Add manual correction from user training (MANUAL LEARNING)
    pub fn add_manual_correction(&self, heard: &str, expected: &str) -> bool {
        let (error_word, correct_word) = self.extract_error_pair(heard, expected);

        if error_word.is_empty() || correct_word.is_empty() {
            return false;
        }

        let mut data = self.data.lock().expect("VoiceFingerprint mutex poisoned");
        let pattern = data
            .asr_patterns
            .entry(error_word.clone())
            .or_insert_with(|| VoicePattern {
                likely_meant: Vec::new(),
                confidence: 0.0,
                count: 0,
                source: "manual".to_string(),
                last_seen: Local::now().to_rfc3339(),
            });

        // Manual patterns get higher weight
        for _ in 0..3 {
            pattern.likely_meant.push(correct_word.clone());
            pattern.count += 1;
        }
        pattern.source = "manual".to_string();
        pattern.last_seen = Local::now().to_rfc3339();

        self.recalculate_confidence(&error_word, pattern);

        drop(data);
        self.save().ok();

        info!(
            "✏️ Manual correction: '{}' -> '{}'",
            error_word, correct_word
        );
        true
    }

    /// Track successful command execution
    pub fn add_successful_command(&self, command: &str) {
        let mut data = self.data.lock().expect("VoiceFingerprint mutex poisoned");
        let count = data
            .command_frequency
            .entry(command.to_lowercase())
            .or_insert(0);
        *count += 1;

        let total: u32 = data.command_frequency.values().sum();
        if total.is_multiple_of(10) {
            drop(data);
            self.save().ok();
        }
    }

    /// Get likely corrections for words in this text
    pub fn get_corrections_for(&self, text: &str) -> HashMap<String, String> {
        let mut corrections = HashMap::new();
        let data = self.data.lock().expect("VoiceFingerprint mutex poisoned");

        for word in text.to_lowercase().split_whitespace() {
            if let Some(pattern) = data.asr_patterns.get(word) {
                if pattern.confidence >= 0.5 && !pattern.likely_meant.is_empty() {
                    // Find most frequent correction
                    let mut counts = HashMap::new();
                    for m in &pattern.likely_meant {
                        *counts.entry(m).or_insert(0) += 1;
                    }
                    if let Some((&best, _)) = counts.iter().max_by_key(|&(_, count)| count) {
                        corrections.insert(word.to_string(), best.clone());
                    }
                }
            }
        }
        corrections
    }

    /// Clear all learned patterns (reset voice fingerprint)
    pub fn clear_patterns(&self) {
        let mut data = self.data.lock().expect("VoiceFingerprint mutex poisoned");
        data.asr_patterns.clear();
        drop(data);
        self.save().ok();
        info!("🗑️ Voice fingerprint cleared");
    }

    /// Get all learned patterns for UI display
    pub fn get_all_patterns(&self) -> HashMap<String, VoicePattern> {
        let data = self.data.lock().expect("VoiceFingerprint mutex poisoned");
        data.asr_patterns.clone()
    }

    /// Get top N most frequently used commands (Python parity)
    pub fn top_commands(&self, n: usize) -> Vec<String> {
        let data = self.data.lock().expect("VoiceFingerprint mutex poisoned");
        let mut sorted: Vec<_> = data.command_frequency.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));
        sorted.into_iter().take(n).map(|(k, _)| k.clone()).collect()
    }

    /// Clear command frequency data (Python parity)
    pub fn clear_commands(&self) {
        let mut data = self.data.lock().expect("VoiceFingerprint mutex poisoned");
        data.command_frequency.clear();
        drop(data);
        self.save().ok();
        info!("🗑️ Command frequency cleared");
    }

    /// Get correction and confidence for a specific word (Python parity)
    pub fn get_correction_with_confidence(&self, word: &str) -> Option<(String, f32)> {
        let data = self.data.lock().expect("VoiceFingerprint mutex poisoned");
        let word_lower = word.to_lowercase();

        if let Some(pattern) = data.asr_patterns.get(&word_lower) {
            if pattern.confidence >= 0.5 && !pattern.likely_meant.is_empty() {
                // Find most frequent correction
                let mut counts = HashMap::new();
                for m in &pattern.likely_meant {
                    *counts.entry(m).or_insert(0) += 1;
                }
                if let Some((&best, _)) = counts.iter().max_by_key(|&(_, count)| count) {
                    return Some((best.clone(), pattern.confidence));
                }
            }
        }
        None
    }

    fn extract_error_pair(&self, asr_text: &str, correct_text: &str) -> (String, String) {
        let as_words: Vec<_> = asr_text
            .to_lowercase()
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
        let co_words: Vec<_> = correct_text
            .to_lowercase()
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        // Simple heuristic: if one side is one word and other side is one word, assume they match
        if as_words.len() == 1 && co_words.len() == 1 {
            return (as_words[0].clone(), co_words[0].clone());
        }

        // Find words unique to each
        let as_set: std::collections::HashSet<_> = as_words.iter().collect();
        let co_set: std::collections::HashSet<_> = co_words.iter().collect();

        let error_candidates: Vec<_> = as_words.iter().filter(|w| !co_set.contains(w)).collect();
        let correct_candidates: Vec<_> = co_words.iter().filter(|w| !as_set.contains(w)).collect();

        if error_candidates.len() == 1 && correct_candidates.len() == 1 {
            return (error_candidates[0].clone(), correct_candidates[0].clone());
        }

        // Fallback: use last word if it differs
        if let (Some(asr_last), Some(co_last)) = (as_words.last(), co_words.last()) {
            return (asr_last.clone(), co_last.clone());
        }

        ("".to_string(), "".to_string())
    }

    fn recalculate_confidence(&self, _word: &str, pattern: &mut VoicePattern) {
        if pattern.likely_meant.is_empty() {
            pattern.confidence = 0.0;
            return;
        }

        let mut counts = HashMap::new();
        for m in &pattern.likely_meant {
            *counts.entry(m).or_insert(0) += 1;
        }

        if let Some((&_best, &best_count)) = counts.iter().max_by_key(|&(_, count)| count) {
            let total_count = pattern.likely_meant.len() as f32;
            let consistency = best_count as f32 / total_count;
            let sample_factor = (total_count / 10.0).sqrt().min(1.0);
            pattern.confidence = (consistency * sample_factor * 100.0).round() / 100.0;
        }
    }

    fn load(&mut self) -> Result<()> {
        if !self.fingerprint_file.exists() {
            return Ok(());
        }
        let content = fs::read_to_string(&self.fingerprint_file)?;
        let data: FingerprintData = serde_json::from_str(&content)?;
        *self.data.lock().expect("VoiceFingerprint mutex poisoned") = data;
        Ok(())
    }

    fn save(&self) -> Result<()> {
        let data = self.data.lock().expect("VoiceFingerprint mutex poisoned");
        let content = serde_json::to_string_pretty(&*data)?;
        let temp_file = self.fingerprint_file.with_extension("tmp");
        fs::write(&temp_file, content)?;
        fs::rename(temp_file, &self.fingerprint_file)?;
        Ok(())
    }

    /// Simulates recording a voice sample for a specific phrase
    pub async fn record_sample(&self, phrase: &str) -> Result<PathBuf> {
        info!("🎙️ Starting recording for phrase: '{}'", phrase);

        // Simulate recording duration
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Sanitize filename
        let safe_name = phrase
            .replace(" ", "_")
            .replace(|c: char| !c.is_alphanumeric() && c != '_', "");
        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        let filename = format!("{}_{}.wav", safe_name, timestamp);
        let file_path = self.base_path.join(filename);

        // Write a mock WAV file (header only + minimal data)
        self.write_dummy_wav(&file_path)?;

        info!("✅ Saved sample to: {:?}", file_path);
        Ok(file_path)
    }

    /// Simulates training the model based on collected samples
    pub async fn train_model(&self) -> Result<()> {
        info!("🧠 Starting model training...");
        tokio::time::sleep(Duration::from_secs(2)).await;
        info!("✅ Model training complete!");
        Ok(())
    }

    fn write_dummy_wav(&self, path: &PathBuf) -> Result<()> {
        // Simple WAV header for 1 second of silence
        let header: [u8; 44] = [
            0x52, 0x49, 0x46, 0x46, // RIFF
            0x24, 0x00, 0x00, 0x00, // ChunkSize
            0x57, 0x41, 0x56, 0x45, // WAVE
            0x66, 0x6d, 0x74, 0x20, // fmt
            0x10, 0x00, 0x00, 0x00, // Subchunk1Size (16 for PCM)
            0x01, 0x00, // AudioFormat (1 = PCM)
            0x01, 0x00, // NumChannels (1)
            0x44, 0xAC, 0x00, 0x00, // SampleRate (44100)
            0x88, 0x58, 0x01, 0x00, // ByteRate
            0x02, 0x00, // BlockAlign
            0x10, 0x00, // BitsPerSample (16)
            0x64, 0x61, 0x74, 0x61, // data
            0x00, 0x00, 0x00, 0x00, // Subchunk2Size (0 data)
        ];
        fs::write(path, header)?;
        Ok(())
    }
}
