//! D-Bus client for speechd-ng integration
//!
//! Uses zbus for async D-Bus communication with the speechd-ng daemon.

use anyhow::Result;
use tracing::{debug, info, warn};
use zbus::{proxy, Connection};

/// D-Bus proxy for the speechd-ng service
#[proxy(
    interface = "org.speech.Service",
    default_service = "org.speech.Service",
    default_path = "/org/speech/Service"
)]
trait SpeechService {
    /// Speak text using default voice
    fn speak(&self, text: &str) -> zbus::Result<()>;

    /// Speak text using specific voice
    fn speak_voice(&self, text: &str, voice: &str) -> zbus::Result<()>;

    /// Listen with VAD and return transcript
    fn listen_vad(&self) -> zbus::Result<String>;

    /// Ping the service
    fn ping(&self) -> zbus::Result<String>;

    /// Get service version
    fn get_version(&self) -> zbus::Result<String>;

    /// Get STT backend name
    fn get_stt_backend(&self) -> zbus::Result<String>;
}

/// Client for communicating with speechd-ng
#[derive(Debug, Clone)]
pub struct SpeechdClient {
    proxy: SpeechServiceProxy<'static>,
}

impl SpeechdClient {
    /// Connect to the speechd-ng D-Bus service
    pub async fn connect() -> Result<Self> {
        let connection = Connection::session().await?;
        let proxy = SpeechServiceProxy::new(&connection).await?;
        
        // Verify connection
        match proxy.ping().await {
            Ok(response) => {
                info!("🔊 Connected to speechd-ng: {}", response);
            }
            Err(e) => {
                warn!("⚠️ speechd-ng not responding: {}", e);
            }
        }

        Ok(Self { proxy })
    }

    /// Speak text using TTS
    pub async fn speak(&self, text: &str) -> Result<()> {
        debug!("Speaking: {}", text);
        self.proxy.speak(text).await?;
        Ok(())
    }

    /// Speak with specific voice
    pub async fn speak_voice(&self, text: &str, voice: &str) -> Result<()> {
        debug!("Speaking with voice {}: {}", voice, text);
        self.proxy.speak_voice(text, voice).await?;
        Ok(())
    }

    /// Listen for speech and return transcript
    pub async fn listen(&self) -> Result<String> {
        debug!("Listening via speechd-ng...");
        let transcript = self.proxy.listen_vad().await?;
        Ok(transcript)
    }

    /// Check if speechd-ng is available
    pub async fn is_available(&self) -> bool {
        self.proxy.ping().await.is_ok()
    }

    /// Get the current STT backend
    pub async fn get_stt_backend(&self) -> Result<String> {
        Ok(self.proxy.get_stt_backend().await?)
    }
}

#[cfg(test)]
mod tests {
    // Integration tests would require a running speechd-ng service
}
