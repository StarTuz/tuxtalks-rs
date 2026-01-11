//! Wyoming Server Manager
//!
//! Manages the lifecycle of the `wyoming-faster-whisper` server process.

use crate::config::Config;
use anyhow::{Context, Result};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

/// Check if a Wyoming server is running on the specified host:port
pub fn is_server_running(host: &str, port: u16) -> bool {
    TcpStream::connect((host, port)).is_ok()
}

/// Start the wyoming-faster-whisper server
pub fn start_server(config: &Config) -> Result<Child> {
    let host = if config.wyoming_host == "localhost" {
        "127.0.0.1"
    } else {
        &config.wyoming_host
    };

    if is_server_running(host, config.wyoming_port) {
        warn!(
            "Wyoming server already running at {}:{}",
            host, config.wyoming_port
        );
        // We can't return a Child handle to a process we didn't start.
        // In a real app we might want to return an Enum (Existing, New(Child)).
        // For now, we'll error out? Or we should check before calling start.
        // But the signature returns Result<Child>.
        // Let's assume the caller checks `is_server_running` first, or we
        // spawn a dummy/check.
        // Actually, Python returns None if running. Here we return Result.
        return Err(anyhow::anyhow!("Server already running"));
    }

    // Check if binary exists
    // This assumes wyoming-faster-whisper is in PATH
    let binary = "wyoming-faster-whisper";

    // Prepare directory
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("tuxtalks/wyoming-data");
    std::fs::create_dir_all(&data_dir).context("Failed to create Wyoming data dir")?;

    let log_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("tuxtalks/logs");
    std::fs::create_dir_all(&log_dir).context("Failed to create logs dir")?;
    let log_file = std::fs::File::create(log_dir.join("wyoming_server.log"))
        .context("Failed to create log file")?;

    info!("🚀 Starting Wyoming Whisper server...");
    info!("   Host: {}:{}", host, config.wyoming_port);
    info!(
        "   Model: {} ({})",
        config.wyoming_model, config.wyoming_compute_type
    );
    info!("   Device: {}", config.wyoming_device);

    let uri = format!("tcp://{}:{}", host, config.wyoming_port);

    let mut child = Command::new(binary)
        .arg("--uri")
        .arg(&uri)
        .arg("--model")
        .arg(&config.wyoming_model)
        .arg("--language")
        .arg("en") // Default to English for now, could be configurable
        .arg("--device")
        .arg(&config.wyoming_device)
        .arg("--compute-type")
        .arg(&config.wyoming_compute_type)
        .arg("--beam-size")
        .arg("1")
        .arg("--data-dir")
        .arg(data_dir)
        .stdout(Stdio::from(log_file.try_clone().unwrap()))
        .stderr(Stdio::from(log_file))
        .spawn()
        .context("Failed to spawn wyoming-faster-whisper. Is it installed?")?;

    // Wait briefly to see if it crashes immediately
    std::thread::sleep(Duration::from_secs_f32(1.5));

    if let Ok(Some(status)) = child.try_wait() {
        error!("Wyoming server exited immediately with status: {}", status);
        return Err(anyhow::anyhow!("Server process exited prematurely"));
    }

    // Wait up to 10s for port to open
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        if is_server_running(host, config.wyoming_port) {
            info!(
                "✅ Wyoming server started successfully (PID: {})",
                child.id()
            );
            return Ok(child);
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    // Timeout
    let _ = child.kill();
    Err(anyhow::anyhow!("Timed out waiting for server to listen"))
}

/// Stop the server process
pub fn stop_server(child: &mut Child) {
    info!("🛑 Stopping Wyoming server (PID: {})...", child.id());

    // Try graceful SIGTERM (Unix only)
    #[cfg(unix)]
    {
        // Simplistic approach: just kill it for now as std::process::Command doesn't expose SIGTERM easily
        // without platform specific extensions. wrapper libraries like `nix` or `libc` usage would be needed.
        // For standard lib, .kill() is SIGKILL.
        // We can try to be nicer if we had `signal-hook` or similar, but .kill() is reliable.
        // Python used .terminate() which is SIGTERM.
        // Rust's .kill() is strictly SIGKILL on Unix usually? No, the documentation says:
        // "On Unix, this sends the SIGKILL signal." -> That's abrupt.
        // "On Windows, this uses TerminateProcess."

        // If we want SIGTERM, we need `nix::sys::signal::kill`.
        // Or assume the user handles it.
        // For parity with "quick and dirty" initially:
    }

    let _ = child.kill();
    let _ = child.wait();
}
