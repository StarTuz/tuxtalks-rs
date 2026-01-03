//! Audio capture module using cpal

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::mpsc::{self, Receiver, Sender};
use tracing::{debug, info, warn};

const SAMPLE_RATE: u32 = 16000;
const CHUNK_SIZE: usize = 1024;

/// Start audio capture and return a receiver for audio chunks
pub fn start_capture(device_index: Option<usize>) -> Result<Receiver<Vec<i16>>> {
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

    let (tx, rx): (Sender<Vec<i16>>, Receiver<Vec<i16>>) = mpsc::channel();

    // Build input stream
    let stream = device.build_input_stream(
        &config,
        move |data: &[i16], _: &cpal::InputCallbackInfo| {
            // Send audio chunk to main thread
            if tx.send(data.to_vec()).is_err() {
                warn!("Audio receiver dropped");
            }
        },
        |err| {
            warn!("Audio stream error: {}", err);
        },
        None,
    )?;

    stream.play()?;

    // Keep stream alive by leaking it (it runs in background)
    // In production, we'd store this in a struct
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_energy_calculation() {
        let silence = vec![0i16; 100];
        assert_eq!(calculate_energy(&silence), 0.0);

        let loud = vec![1000i16; 100];
        assert!(calculate_energy(&loud) > 0.0);
    }
}
