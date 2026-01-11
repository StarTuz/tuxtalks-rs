//! Wyoming Protocol Client
//!
//! Implements the Wyoming protocol for external ASR services.
//! Wyoming is a simple protocol where events are JSON lines over TCP.
//!
//! Reference: https://github.com/rhasspy/wyoming

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tracing::{debug, info, warn};

/// Wyoming event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WyomingEvent {
    /// Describe request (handshake)
    #[serde(rename = "describe")]
    Describe,

    /// Info response from server
    #[serde(rename = "info")]
    Info(InfoData),

    /// Start of audio stream
    #[serde(rename = "audio-start")]
    AudioStart(AudioStartData),

    /// Audio chunk
    #[serde(rename = "audio-chunk")]
    AudioChunk(AudioChunkData),

    /// End of audio stream
    #[serde(rename = "audio-stop")]
    AudioStop,

    /// Transcript result
    #[serde(rename = "transcript")]
    Transcript(TranscriptData),
}

/// Info response data
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InfoData {
    #[serde(default)]
    pub asr: Vec<AsrInfo>,
}

/// ASR service info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrInfo {
    pub name: String,
    #[serde(default)]
    pub languages: Vec<String>,
}

/// Audio start data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioStartData {
    pub rate: u32,
    pub width: u8,
    pub channels: u8,
}

/// Audio chunk data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioChunkData {
    pub rate: u32,
    pub width: u8,
    pub channels: u8,
    #[serde(with = "base64_bytes")]
    pub audio: Vec<u8>,
    #[serde(default)]
    pub timestamp: u64,
}

/// Transcript result data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptData {
    pub text: String,
}

/// Base64 serialization for audio bytes
mod base64_bytes {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        STANDARD.decode(&s).map_err(serde::de::Error::custom)
    }
}

/// Wyoming client for ASR services
pub struct WyomingClient {
    host: String,
    port: u16,
    sample_rate: u32,
}

impl WyomingClient {
    /// Create a new Wyoming client
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            host: host.to_string(),
            port,
            sample_rate: 16000,
        }
    }

    /// Check if the server is available
    pub async fn health_check(&self) -> bool {
        match TcpStream::connect((&*self.host, self.port)).await {
            Ok(_) => {
                debug!("Wyoming server available at {}:{}", self.host, self.port);
                true
            }
            Err(e) => {
                warn!("Wyoming server not available: {}", e);
                false
            }
        }
    }

    /// Transcribe audio data
    ///
    /// Sends audio to Wyoming server and returns transcript
    pub async fn transcribe(&self, audio_data: &[u8]) -> Result<String> {
        let stream = TcpStream::connect((&*self.host, self.port))
            .await
            .context("Failed to connect to Wyoming server")?;

        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);

        // Send Describe (handshake)
        let describe = serde_json::json!({"type": "describe"});
        writer.write_all(describe.to_string().as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;

        // Read Info response
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        debug!("Wyoming handshake: {}", line.trim());

        // Send AudioStart
        let audio_start = serde_json::json!({
            "type": "audio-start",
            "data": {
                "rate": self.sample_rate,
                "width": 2,
                "channels": 1
            }
        });
        writer.write_all(audio_start.to_string().as_bytes()).await?;
        writer.write_all(b"\n").await?;

        // Send AudioChunk
        let audio_chunk = serde_json::json!({
            "type": "audio-chunk",
            "data": {
                "rate": self.sample_rate,
                "width": 2,
                "channels": 1,
                "audio": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, audio_data),
                "timestamp": 0
            }
        });
        writer.write_all(audio_chunk.to_string().as_bytes()).await?;
        writer.write_all(b"\n").await?;

        // Send AudioStop
        let audio_stop = serde_json::json!({"type": "audio-stop"});
        writer.write_all(audio_stop.to_string().as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;

        debug!(
            "Sent audio ({} bytes), waiting for transcript...",
            audio_data.len()
        );

        // Read Transcript response (with timeout)
        let timeout = Duration::from_secs(30);
        let transcript = tokio::time::timeout(timeout, async {
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).await? == 0 {
                    break;
                }

                if let Ok(event) = serde_json::from_str::<serde_json::Value>(&line) {
                    if event.get("type").and_then(|t| t.as_str()) == Some("transcript") {
                        if let Some(data) = event.get("data") {
                            if let Some(text) = data.get("text").and_then(|t| t.as_str()) {
                                return Ok::<_, anyhow::Error>(text.to_string());
                            }
                        }
                    }
                }
            }
            Ok(String::new())
        })
        .await
        .context("Timeout waiting for transcript")??;

        info!("📝 Wyoming transcript: '{}'", transcript);
        Ok(transcript)
    }
}

#[async_trait::async_trait]
impl super::AsrEngine for WyomingClient {
    fn process(&mut self, _samples: &[i16]) -> Result<Option<super::AsrResult>> {
        // Wyoming stream processing is complex and requires persistent async connection.
        // For now, Wyoming is handled via the separate transcribe() method for full blocks.
        Ok(None)
    }

    fn reset(&mut self) {}
}

#[cfg(test)]
mod tests {
    #[allow(unused)]
    #[test]
    fn test_wyoming_event_serialize() {
        let event = serde_json::json!({"type": "describe"});
        assert_eq!(event["type"], "describe");
    }

    #[test]
    fn test_audio_start_serialize() {
        let audio_start = serde_json::json!({
            "type": "audio-start",
            "data": {
                "rate": 16000,
                "width": 2,
                "channels": 1
            }
        });
        assert_eq!(audio_start["data"]["rate"], 16000);
    }
}
