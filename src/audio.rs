//! Audio capture module using cpal
//!
//! Captures audio from the default input device and sends it to a channel.

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

const SAMPLE_RATE: u32 = 16000;
const CHUNK_SIZE: usize = 1024;

/// Start audio capture and return a receiver for audio chunks
pub fn start_capture(device_index: Option<usize>) -> Result<mpsc::UnboundedReceiver<Vec<i16>>> {
    let host = cpal::default_host();

    // List available devices
    info!("Available audio input devices:");
    for (i, device) in host.input_devices()?.enumerate() {
        let name = device.name().unwrap_or_else(|_| "Unknown".to_string());
        let marker = if device_index == Some(i) { "*" } else { " " };
        info!("  {} [{}] {}", marker, i, name);
    }

    // Select device
    let device = if let Some(idx) = device_index {
        host.input_devices()?
            .nth(idx)
            .context("Device index out of range")?
    } else {
        host.default_input_device()
            .context("No default input device")?
    };

    let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
    info!("Using audio device: {}", device_name);

    // Configure stream
    let config = cpal::StreamConfig {
        channels: 1,
        sample_rate: cpal::SampleRate(SAMPLE_RATE),
        buffer_size: cpal::BufferSize::Fixed(CHUNK_SIZE as u32),
    };

    let (tx, rx) = mpsc::unbounded_channel();

    // Build input stream
    let stream = device.build_input_stream(
        &config,
        move |data: &[i16], _: &cpal::InputCallbackInfo| {
            // Send audio chunk - Unbounded so it won't block the audio thread
            if let Err(_) = tx.send(data.to_vec()) {
                // If receiver is dropped, this is fine when shutting down
            }
        },
        |err| {
            warn!("Audio stream error: {}", err);
        },
        None,
    )?;

    stream.play()?;

    // Keep stream alive by leaking it (it runs in background)
    // In production, we'd store this in a struct in the App state
    // but for now we follow the same pattern for simplicity.
    std::mem::forget(stream);

    Ok(rx)
}

/// Calculate audio energy for VAD
pub fn calculate_energy(samples: &[i16]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    let sum: i64 = samples.iter().map(|&s| (s as i64).pow(2)).sum();
    (sum as f32 / samples.len() as f32).sqrt()
}
