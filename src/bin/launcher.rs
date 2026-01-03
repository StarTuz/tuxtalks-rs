//! TuxTalks Launcher - GUI Application
//!
//! Run with: cargo run --bin tuxtalks-launcher

use iced::application;

// Import from the library
use tuxtalks::gui::TuxTalksApp;

fn main() -> iced::Result {
    application("TuxTalks", TuxTalksApp::update, TuxTalksApp::view)
        .theme(TuxTalksApp::theme)
        .subscription(TuxTalksApp::subscription)
        .run_with(TuxTalksApp::new)
}
