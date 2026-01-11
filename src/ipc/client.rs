//! IPC Client
//!
//! Unix socket client for launcher-side IPC.

use anyhow::Result;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tracing::{debug, warn};

use super::{socket_path, IpcRequest, IpcResponse};

/// IPC Client for launcher
pub struct IpcClient;

static NEXT_SEQ_ID: AtomicU64 = AtomicU64::new(1);

fn next_seq_id() -> u64 {
    NEXT_SEQ_ID.fetch_add(1, Ordering::SeqCst)
}

impl IpcClient {
    /// Check if the daemon is running
    pub fn is_daemon_running() -> bool {
        let path = socket_path();

        if !path.exists() {
            return false;
        }

        UnixStream::connect(path).is_ok()
    }

    /// Send a selection request to the daemon
    pub fn send_selection_request(
        title: &str,
        items: Vec<String>,
        page: usize,
        timeout: Duration,
    ) -> Result<Option<(i32, bool)>> {
        let path = socket_path();

        let mut stream = UnixStream::connect(&path)?;
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;

        let seq_id = next_seq_id();
        let request = IpcRequest::SelectionRequest {
            seq_id,
            title: title.to_string(),
            items,
            page,
        };

        let request_json = serde_json::to_string(&request)? + "\n";
        stream.write_all(request_json.as_bytes())?;

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line)?;

        let response: IpcResponse = serde_json::from_str(line.trim())?;
        debug!("📨 IPC response: {:?}", response);

        match response {
            IpcResponse::SelectionResponse {
                seq_id: resp_seq,
                index,
                cancelled,
                ..
            } => {
                if resp_seq != seq_id {
                    warn!(
                        "⚠️ IPC sequence ID mismatch: expected {}, got {}",
                        seq_id, resp_seq
                    );
                    return Ok(None);
                }
                Ok(Some((index, cancelled)))
            }
            _ => {
                warn!("Unexpected IPC response type");
                Ok(None)
            }
        }
    }

    /// Request daemon status
    pub fn get_status() -> Result<Option<(bool, bool, Option<String>)>> {
        let path = socket_path();

        let mut stream = UnixStream::connect(&path)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;

        let seq_id = next_seq_id();
        let request = IpcRequest::StatusRequest { seq_id };
        let request_json = serde_json::to_string(&request)? + "\n";
        stream.write_all(request_json.as_bytes())?;

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line)?;

        let response: IpcResponse = serde_json::from_str(line.trim())?;

        match response {
            IpcResponse::StatusResponse {
                seq_id: resp_seq,
                listening,
                paused,
                active_profile,
            } => {
                if resp_seq != seq_id {
                    warn!("⚠️ IPC sequence ID mismatch");
                    return Ok(None);
                }
                Ok(Some((listening, paused, active_profile)))
            }
            _ => Ok(None),
        }
    }

    /// Send a control command
    pub fn send_control(action: &str) -> Result<bool> {
        let path = socket_path();

        let mut stream = UnixStream::connect(&path)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;

        let seq_id = next_seq_id();
        let request = IpcRequest::Control {
            seq_id,
            action: action.to_string(),
        };
        let request_json = serde_json::to_string(&request)? + "\n";
        stream.write_all(request_json.as_bytes())?;

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line)?;

        let response: IpcResponse = serde_json::from_str(line.trim())?;

        match response {
            IpcResponse::Ack {
                seq_id: resp_seq,
                success,
                ..
            } => {
                if resp_seq != seq_id {
                    warn!("⚠️ IPC sequence ID mismatch");
                    return Ok(false);
                }
                Ok(success)
            }
            _ => Ok(false),
        }
    }
}
