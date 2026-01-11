//! Ollama AI Integration
//!
//! Provides natural language understanding for TuxTalks voice commands.
//! Routes commands through Ollama LLM for intent extraction.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

/// Intent extracted from natural language
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intent {
    pub name: String,
    pub parameters: std::collections::HashMap<String, String>,
    pub confidence: f32,
}

/// Ollama API response
#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
}

/// Handles Ollama LLM integration for intent extraction
#[derive(Clone)]
pub struct OllamaHandler {
    url: String,
    model: String,
    enabled: bool,
}

impl OllamaHandler {
    /// Create new Ollama handler from config
    pub fn new(config: &crate::config::Config) -> Self {
        Self {
            url: config.ollama_url.clone(),
            model: config.ollama_model.clone(),
            enabled: config.ollama_enabled,
        }
    }

    /// Check if Ollama is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Health check - verify Ollama is reachable
    pub async fn health_check(&self) -> bool {
        if !self.enabled {
            return false;
        }

        let client = reqwest::Client::new();
        match client
            .get(format!("{}/api/tags", self.url))
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
        {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    /// Extract intent from natural language text
    pub async fn extract_intent(
        &self,
        text: &str,
        corrections: &std::collections::HashMap<String, String>,
    ) -> Result<Option<Intent>> {
        if !self.enabled {
            return Ok(None);
        }

        let prompt = self.build_prompt(text, corrections);

        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/api/generate", self.url))
            .json(&serde_json::json!({
                "model": self.model,
                "prompt": prompt,
                "stream": false,
                "options": {
                    "temperature": 0.1,
                    "num_predict": 150
                }
            }))
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await?;

        let status = response.status();
        let body_text = response.text().await?;

        if !status.is_success() {
            warn!("❌ Ollama API Error ({}): {}", status, body_text);
            return Ok(None);
        }

        debug!("🧠 Ollama raw body: {}", body_text);

        let ollama_resp: OllamaResponse = match serde_json::from_str(&body_text) {
            Ok(r) => r,
            Err(e) => {
                warn!(
                    "❌ Failed to deserialize Ollama response: {} - Body: {}",
                    e, body_text
                );
                return Ok(None);
            }
        };

        // Parse the JSON inside the 'response' field
        self.parse_intent_response(&ollama_resp.response)
    }

    fn build_prompt(
        &self,
        text: &str,
        corrections: &std::collections::HashMap<String, String>,
    ) -> String {
        let mut correction_hint = String::new();
        if !corrections.is_empty() {
            correction_hint =
                "\nNote these frequent voice recognition errors and their likely meanings:\n"
                    .to_string();
            for (error, correct) in corrections {
                correction_hint.push_str(&format!("- \"{error}\" likely means \"{correct}\"\n"));
            }
        }

        format!(
            r#"You are a voice command parser. Extract the intent from this command and respond with ONLY valid JSON.
{correction_hint}
Command: "{text}"

Respond with JSON in this exact format:
{{"intent": "intent_name", "parameters": {{"param": "value"}}, "confidence": 0.9}}

Valid intents:
- play_artist: Play music by an artist. Params: artist
- play_album: Play an album. Params: album, artist (optional)
- play_song: Play a specific song. Params: song, artist (optional)
- play_playlist: Play a playlist. Params: playlist
- media_control: Pause, stop, next, previous. Params: action
- volume_control: Volume up/down/set. Params: action, level (optional)
- game_command: Gaming voice command. Params: command
- unknown: Cannot determine intent

JSON response:"#
        )
    }

    fn parse_intent_response(&self, response: &str) -> Result<Option<Intent>> {
        // Find JSON in response (Ollama may include extra text)
        let json_start = response.find('{');
        let json_end = response.rfind('}');

        if let (Some(start), Some(end)) = (json_start, json_end) {
            let json_str = &response[start..=end];

            #[derive(Deserialize)]
            struct ParsedIntent {
                intent: String,
                parameters: std::collections::HashMap<String, String>,
                confidence: f32,
            }

            match serde_json::from_str::<ParsedIntent>(json_str) {
                Ok(parsed) => {
                    if parsed.intent == "unknown" {
                        return Ok(None);
                    }
                    Ok(Some(Intent {
                        name: parsed.intent,
                        parameters: parsed.parameters,
                        confidence: parsed.confidence,
                    }))
                }
                Err(e) => {
                    warn!(
                        "❌ Failed to parse Ollama response: {} - Raw: {}",
                        e, json_str
                    );
                    Ok(None)
                }
            }
        } else {
            debug!("No JSON found in Ollama response: {}", response);
            Ok(None)
        }
    }

    /// Learn from successful Ollama correction (passive learning)
    ///
    /// Detects when Ollama successfully corrected an ASR error and
    /// can teach the voice fingerprint.
    ///
    /// Example:
    ///   User says "Abba" → ASR hears "play ever"
    ///   → Ollama extracts artist="abba"
    ///   → Music plays successfully
    ///   → System learns: "ever" → "abba"
    pub fn learn_from_success(
        &self,
        original_asr: &str,
        intent: &Intent,
    ) -> Option<(String, String)> {
        // Check if Ollama extracted something NOT in the ASR output
        let entity_params = ["artist", "album", "song", "genre", "playlist"];

        for param_name in entity_params {
            if let Some(extracted) = intent.parameters.get(param_name) {
                let extracted_lower = extracted.to_lowercase();
                let asr_lower = original_asr.to_lowercase();

                // If extracted entity is NOT in ASR output, it's a correction
                if !extracted_lower.is_empty() && !asr_lower.contains(&extracted_lower) {
                    debug!(
                        "🎯 Correction detected! ASR='{}' but Ollama extracted '{}'",
                        original_asr, extracted
                    );
                    return Some((original_asr.to_string(), extracted.clone()));
                }
            }
        }

        None
    }
}
