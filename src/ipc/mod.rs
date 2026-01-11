//! IPC (Inter-Process Communication) Module
//!
//! Unix socket-based communication between TuxTalks daemon and launcher.
//! Protocol: JSON over newline-delimited messages.

pub mod client;
pub mod messages;
pub mod server;

pub use client::IpcClient;
pub use messages::*;
pub use server::IpcServer;

use std::path::PathBuf;

/// Get the Unix socket path for IPC
pub fn socket_path() -> PathBuf {
    // Use username for socket path (matches Python: /tmp/tuxtalks-menu-{uid}.sock)
    let user = std::env::var("USER").unwrap_or_else(|_| "tuxtalks".to_string());
    PathBuf::from(format!("/tmp/tuxtalks-{}.sock", user))
}
