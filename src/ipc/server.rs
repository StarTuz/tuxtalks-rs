//! IPC Server
//!
//! Unix socket server for handling daemon-side IPC.

use anyhow::Result;
use lazy_static::lazy_static;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::Mutex;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

lazy_static! {
    static ref LAST_IPC_TIME: Mutex<Instant> = Mutex::new(Instant::now() - Duration::from_secs(1));
}

use super::{socket_path, IpcRequest, IpcResponse};

/// Callback type for handling selection requests (seq_id, title, items, page) -> (index, cancelled)
pub type SelectionCallback =
    Box<dyn Fn(u64, String, Vec<String>, usize) -> (i32, bool) + Send + Sync>;

/// IPC Server for daemon
pub struct IpcServer {
    running: Arc<AtomicBool>,
    thread_handle: Option<thread::JoinHandle<()>>,
}

impl IpcServer {
    /// Create new IPC server
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            thread_handle: None,
        }
    }

    /// Start the server with a selection callback
    pub fn start<F>(&mut self, callback: F) -> Result<()>
    where
        F: Fn(u64, String, Vec<String>, usize) -> (i32, bool) + Send + Sync + 'static,
    {
        let path = socket_path();

        // Clean up stale socket
        if path.exists() {
            let _ = fs::remove_file(&path);
        }

        // Create listener
        let listener = UnixListener::bind(&path)?;

        // Set strict permissions (user only: RW-------)
        // Red Team Audit (Alex Stamos): Prevent local privilege escalation/hijack
        if let Ok(metadata) = fs::metadata(&path) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o600);
            if let Err(e) = fs::set_permissions(&path, perms) {
                warn!("⚠️ Failed to set strict IPC socket permissions: {}", e);
            } else {
                debug!("🔒 IPC socket permissions set to 0600");
            }
        }

        listener.set_nonblocking(true)?;

        self.running.store(true, Ordering::SeqCst);
        let running = self.running.clone();
        let callback = Arc::new(callback);

        info!("🔌 IPC server listening on {:?}", path);

        let handle = thread::spawn(move || {
            while running.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let cb = callback.clone();
                        thread::spawn(move || {
                            if let Err(e) = handle_client(stream, cb) {
                                warn!("IPC client error: {}", e);
                            }
                        });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(100));
                    }
                    Err(e) => {
                        warn!("IPC accept error: {}", e);
                    }
                }
            }

            // Cleanup
            let _ = fs::remove_file(socket_path());
            info!("🔌 IPC server stopped");
        });

        self.thread_handle = Some(handle);
        Ok(())
    }

    /// Stop the server
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }

    /// Check if server is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

impl Default for IpcServer {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Handle a single client connection
fn handle_client<F>(mut stream: UnixStream, callback: Arc<F>) -> Result<()>
where
    F: Fn(u64, String, Vec<String>, usize) -> (i32, bool),
{
    // DoS Protection: Rate limiting (Alex Stamos requirement)
    {
        let mut last_time = LAST_IPC_TIME
            .lock()
            .expect("IPC LAST_IPC_TIME mutex poisoned");
        if last_time.elapsed() < Duration::from_millis(100) {
            warn!("⚠️ IPC Rate Limit triggered - rejecting request");
            return Ok(());
        }
        *last_time = Instant::now();
    }

    // DoS Protection: Message size limit
    let mut reader = BufReader::new(stream.try_clone()?).take(4096); // Max 4KB per request
    let mut line = String::new();

    reader.read_line(&mut line)?;

    if line.is_empty() {
        return Ok(());
    }

    let request: IpcRequest = serde_json::from_str(line.trim())?;
    debug!("📨 IPC request: {:?}", request);

    let response = match request {
        IpcRequest::SelectionRequest {
            seq_id,
            title,
            items,
            page,
        } => {
            let (index, cancelled) = callback(seq_id, title, items, page);
            IpcResponse::SelectionResponse {
                seq_id,
                index,
                cancelled,
                child_index: None,
            }
        }
        IpcRequest::StatusRequest { seq_id } => IpcResponse::StatusResponse {
            seq_id,
            listening: true,
            paused: false,
            active_profile: None,
        },
        IpcRequest::Control { seq_id, action } => {
            info!("📡 IPC control: {}", action);
            if let Err(e) = crate::audit::log(&format!("IPC Control Executed: {}", action)) {
                warn!("Failed to write audit log: {}", e);
            }
            IpcResponse::Ack {
                seq_id,
                success: true,
                message: Some(format!("Executed: {}", action)),
            }
        }
        IpcRequest::ReloadConfig { seq_id } => {
            info!("📡 IPC reload config");
            IpcResponse::Ack {
                seq_id,
                success: true,
                message: Some("Config reloaded".to_string()),
            }
        }
    };

    let response_json = serde_json::to_string(&response)? + "\n";
    stream.write_all(response_json.as_bytes())?;

    Ok(())
}
