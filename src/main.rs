//! TuxTalks - Voice Control for Linux Gaming
//!
//! A Rust implementation of the TuxTalks voice assistant.

mod asr;
mod audio;
mod commands;
mod config;
mod games;
mod gui;
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
    let mut audio_rx = audio::start_capture(args.device)?;
    info!("🎙️ Audio capture started");

    // Initialize ASR
    let mut asr = asr::VoskAsr::new()?;
    // Initialize command processor
    let mut processor = CommandProcessor::new()?;

    // Initialize game manager
    let mut game_manager = games::GameManager::new()?;
    if let Some(idx) = game_manager.detect_active_profile() {
        let profile = &game_manager.profiles[idx];
        info!("🎯 Auto-detected active game: {}", profile.name);
        
        let commands = profile.get_processor_commands();
        for cmd in commands {
            processor.add_command(cmd);
        }
        processor.set_action_map(profile.resolve_actions());
    } else {
        info!("💡 No active game detected, using demo bindings");
        processor.add_demo_bindings();
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

    while let Some(samples) = audio_rx.recv().await {
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

    Ok(())
}
