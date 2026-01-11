//! Sound Engine for audio feedback
//!
//! Uses a channel-based architecture to handle rodio's non-Send stream.
//! The engine spawns a dedicated audio thread that owns the playback infrastructure.

use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub enum PlaybackMode {
    #[default]
    Random,
    Simultaneous,
    Sequential,
}

/// Commands sent to the audio thread
enum AudioCommand {
    PlayFile(PathBuf),
    PlayWait(PathBuf, mpsc::Sender<()>),
    Stop,
    PlayPool {
        pool_id: String,
        files: Vec<PathBuf>,
        mode: PlaybackMode,
    },
}

/// Thread-safe handle to the sound engine
#[derive(Clone)]
pub struct SoundEngine {
    sender: mpsc::Sender<AudioCommand>,
}

impl std::fmt::Debug for SoundEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoundEngine").finish()
    }
}

impl SoundEngine {
    pub fn new() -> anyhow::Result<Self> {
        let (sender, receiver) = mpsc::channel::<AudioCommand>();

        // Spawn dedicated audio thread
        thread::spawn(move || {
            Self::audio_thread(receiver);
        });

        Ok(Self { sender })
    }

    fn audio_thread(receiver: mpsc::Receiver<AudioCommand>) {
        use rodio::OutputStream;
        use std::collections::HashMap;

        // Initialize audio output on this thread
        let (stream, stream_handle) = match OutputStream::try_default() {
            Ok(s) => s,
            Err(e) => {
                warn!("🔇 Failed to initialize audio output: {}", e);
                return;
            }
        };

        // Keep stream alive
        let _stream = stream;
        let mut sink = match rodio::Sink::try_new(&stream_handle) {
            Ok(s) => s,
            Err(e) => {
                error!("❌ Failed to create audio sink: {}", e);
                return;
            }
        };
        let mut pool_states: HashMap<String, usize> = HashMap::new();

        info!("🔊 Audio thread started");

        while let Ok(cmd) = receiver.recv() {
            match cmd {
                AudioCommand::PlayFile(path) => {
                    info!("🔊 Playing file: {:?}", path);
                    if let Err(e) = Self::play_file_internal_sink(&sink, &path) {
                        error!("❌ Audio playback failed for {:?}: {}", path, e);
                    }
                }
                AudioCommand::PlayWait(path, resp) => {
                    info!("🔊 Playing file (blocking): {:?}", path);
                    if let Err(e) = Self::play_file_internal_sink(&sink, &path) {
                        error!("❌ Audio sync playback failed for {:?}: {}", path, e);
                    }
                    sink.sleep_until_end();
                    let _ = resp.send(());
                }
                AudioCommand::Stop => {
                    info!("🛑 Stopping all playback");
                    sink.stop();
                    // Re-create sink after stop as it becomes unusable if we want to play again
                    if let Ok(new_sink) = rodio::Sink::try_new(&stream_handle) {
                        sink = new_sink;
                    }
                }
                AudioCommand::PlayPool {
                    pool_id,
                    files,
                    mode,
                } => {
                    if files.is_empty() {
                        warn!("⚠️ Audio pool '{}' is empty", pool_id);
                        continue;
                    }

                    info!(
                        "🔊 Playing pool '{}' ({} files, mode: {:?})",
                        pool_id,
                        files.len(),
                        mode
                    );
                    match mode {
                        PlaybackMode::Random => {
                            let mut rng = rand::thread_rng();
                            if let Some(file) = files.choose(&mut rng) {
                                debug!("  - Randomly matched: {:?}", file);
                                if let Err(e) = Self::play_file_internal_sink(&sink, file) {
                                    error!("❌ Failed to play random file {:?}: {}", file, e);
                                }
                            }
                        }
                        PlaybackMode::Simultaneous => {
                            for file in &files {
                                debug!("  - Simultaneous: {:?}", file);
                                if let Err(e) = Self::play_file_internal_sink(&sink, file) {
                                    error!("❌ Failed to play simultaneous file {:?}: {}", file, e);
                                }
                            }
                        }
                        PlaybackMode::Sequential => {
                            let idx = pool_states.entry(pool_id).or_insert(0);
                            let wrapped_idx = *idx % files.len();
                            let file = &files[wrapped_idx];
                            debug!("  - Sequential ({}): {:?}", wrapped_idx, file);
                            if let Err(e) = Self::play_file_internal_sink(&sink, file) {
                                error!("❌ Failed to play sequential file {:?}: {}", file, e);
                            }
                            *idx = wrapped_idx + 1;
                        }
                    }
                }
            }
        }

        info!("🔇 Audio thread stopped");
    }

    fn play_file_internal_sink(sink: &rodio::Sink, path: &PathBuf) -> anyhow::Result<()> {
        use rodio::Decoder;
        use std::fs::File;
        use std::io::BufReader;

        if !path.exists() {
            anyhow::bail!("Audio file not found: {:?}", path);
        }

        let file = File::open(path)?;
        let source = Decoder::new(BufReader::new(file))?;

        sink.append(source);

        info!("🔊 Queueing: {:?}", path.file_name().unwrap_or_default());
        Ok(())
    }

    /// Play a single audio file (Async)
    pub fn play_file<P: Into<PathBuf>>(&self, path: P) -> anyhow::Result<()> {
        self.sender
            .send(AudioCommand::PlayFile(path.into()))
            .map_err(|e| anyhow::anyhow!("Audio thread disconnected: {}", e))
    }

    /// Play a single audio file and wait for completion (Sync/Blocking)
    pub fn play_file_sync<P: Into<PathBuf>>(&self, path: P) -> anyhow::Result<()> {
        let (tx, rx) = mpsc::channel();
        self.sender
            .send(AudioCommand::PlayWait(path.into(), tx))
            .map_err(|e| anyhow::anyhow!("Audio thread disconnected: {}", e))?;

        let _ = rx.recv();
        Ok(())
    }

    /// Stop all current playback and clear queue
    pub fn stop(&self) -> anyhow::Result<()> {
        self.sender
            .send(AudioCommand::Stop)
            .map_err(|e| anyhow::anyhow!("Audio thread disconnected: {}", e))
    }

    /// Play a sound pool
    pub fn play_pool(
        &self,
        pool_id: &str,
        files: Vec<PathBuf>,
        mode: PlaybackMode,
    ) -> anyhow::Result<()> {
        self.sender
            .send(AudioCommand::PlayPool {
                pool_id: pool_id.to_string(),
                files,
                mode,
            })
            .map_err(|e| anyhow::anyhow!("Audio thread disconnected: {}", e))
    }
}
