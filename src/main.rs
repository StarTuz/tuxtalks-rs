//! TuxTalks - Voice Control for Linux Gaming
//!
//! A Rust implementation of the TuxTalks voice assistant.

mod asr;
mod audio;
mod commands;
mod config;
mod input;
mod speechd;

use anyhow::Result;
use clap::Parser;
use commands::CommandProcessor;
use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Audio input device index
    #[arg(short, long)]
    device: Option<usize>,

    /// Use speechd-ng for TTS feedback
    #[arg(long)]
    speechd: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Setup logging
    let level = if args.verbose { Level::DEBUG } else { Level::INFO };
    let subscriber = FmtSubscriber::builder().with_max_level(level).finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("🐧 TuxTalks v{} starting...", env!("CARGO_PKG_VERSION"));

    // Initialize audio capture
    let audio_rx = audio::start_capture(args.device)?;
    info!("🎙️ Audio capture started");

    // Initialize ASR
    let mut asr = asr::VoskAsr::new()?;
    info!("🗣️ ASR engine ready");

    // Initialize command processor
    let mut processor = CommandProcessor::new()?;
    processor.add_demo_bindings();

    if !processor.has_keyboard() {
        warn!("⚠️ Running without keyboard simulation");
        warn!("   Voice commands will be recognized but not executed");
    }

    // Optionally connect to speechd-ng for TTS
    let speechd_client = if args.speechd {
        match speechd::SpeechdClient::connect().await {
            Ok(client) => {
                client.speak("TuxTalks ready").await.ok();
                Some(client)
            }
            Err(e) => {
                warn!("Could not connect to speechd-ng: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Main loop
    info!("✅ TuxTalks ready - speak a command");
    info!("   Try: 'boost', 'fire', 'pause', 'screenshot', 'quick save'");

    loop {
        // Get audio chunk from capture thread
        if let Ok(samples) = audio_rx.recv() {
            // Feed to ASR
            if let Some(text) = asr.process(&samples)? {
                if !text.is_empty() {
                    info!("📝 Heard: '{}'", text);

                    // Process command
                    if let Some(cmd) = processor.process(&text) {
                        // Speak feedback if speechd available
                        if let Some(ref client) = speechd_client {
                            client.speak(&format!("Executing {}", cmd)).await.ok();
                        }
                    }
                }
            }
        }
    }
}
