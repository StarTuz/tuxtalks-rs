//! TuxTalks Launcher - GUI Application
//!
//! Run with: cargo run --bin tuxtalks-launcher

use iced::{application, Settings};

// Import from the library
use tuxtalks::gui::TuxTalksApp;

fn main() -> iced::Result {
    application("TuxTalks", TuxTalksApp::update, TuxTalksApp::view)
        .theme(TuxTalksApp::theme)
        .run_with(TuxTalksApp::new)
}
