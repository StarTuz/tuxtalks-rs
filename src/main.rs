//! TuxTalks - Voice Control for Linux Gaming
//! 
//! A Rust implementation of the TuxTalks voice assistant.

mod audio;
mod asr;
mod config;

use anyhow::Result;
use clap::Parser;
use tracing::{info, Level};
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
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Setup logging
    let level = if args.verbose { Level::DEBUG } else { Level::INFO };
    let subscriber = FmtSubscriber::builder()
        .with_max_level(level)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("🐧 TuxTalks v{} starting...", env!("CARGO_PKG_VERSION"));

    // Initialize audio capture
    let audio_rx = audio::start_capture(args.device)?;
    info!("🎙️ Audio capture started");

    // Initialize ASR
    let mut asr = asr::VoskAsr::new()?;
    info!("🗣️ ASR engine ready");

    // Main loop
    info!("✅ TuxTalks ready - speak a command");
    
    loop {
        // Get audio chunk from capture thread
        if let Ok(samples) = audio_rx.recv() {
            // Feed to ASR
            if let Some(text) = asr.process(&samples)? {
                if !text.is_empty() {
                    info!("📝 Heard: '{}'", text);
                    // TODO: Process command
                }
            }
        }
    }
}
