//! Speechd-ng TTS backend using D-Bus

use crate::tts::TtsEngine;
use anyhow::Result;
use async_trait::async_trait;
use tracing::{info, warn};
use zbus::{proxy, Connection};

#[proxy(
    interface = "org.speech.Service",
    default_service = "org.speech.Service",
    default_path = "/org/speech/Service"
)]
trait SpeechService {
    fn speak(&self, text: &str) -> zbus::Result<()>;
    fn ping(&self) -> zbus::Result<String>;
}

pub struct SpeechdEngine {
    proxy: SpeechServiceProxy<'static>,
}

impl std::fmt::Debug for SpeechdEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpeechdEngine").finish()
    }
}

impl SpeechdEngine {
    pub async fn connect() -> Result<Self> {
        let connection = Connection::session().await?;
        let proxy = SpeechServiceProxy::new(&connection).await?;

        // Match the ping logic from the original speechd.rs
        match proxy.ping().await {
            Ok(response) => {
                info!("🔊 Connected to speechd-ng: {}", response);
            }
            Err(e) => {
                warn!("⚠️ speechd-ng not responding: {}", e);
                return Err(anyhow::anyhow!("speechd-ng not responding: {}", e));
            }
        }

        Ok(Self { proxy })
    }
}

#[async_trait]
impl TtsEngine for SpeechdEngine {
    async fn speak(&self, text: &str) -> Result<()> {
        self.proxy.speak(text).await?;
        Ok(())
    }

    fn name(&self) -> &str {
        "speechd_ng"
    }
}
