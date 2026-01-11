//! TuxTalks Launcher - GUI Application
//!
//! Run with: cargo run --bin tuxtalks-launcher

use iced::application;

// Import from the library
use tuxtalks::gui::TuxTalksApp;

fn main() -> iced::Result {
    // Setup logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    application("TuxTalks", TuxTalksApp::update, TuxTalksApp::view)
        .theme(TuxTalksApp::theme)
        .subscription(TuxTalksApp::subscription)
        .run_with(TuxTalksApp::new)
}
