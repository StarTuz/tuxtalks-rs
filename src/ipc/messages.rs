//! IPC Message Types
//!
//! JSON-serializable messages for daemon ↔ launcher communication.

use serde::{Deserialize, Serialize};

/// Request types sent from client to server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcRequest {
    /// Request to show a selection menu
    #[serde(rename = "selection_request")]
    SelectionRequest {
        seq_id: u64,
        title: String,
        items: Vec<String>,
        page: usize,
    },

    /// Request status of the daemon
    #[serde(rename = "status_request")]
    StatusRequest { seq_id: u64 },

    /// Control command (pause, resume, stop)
    #[serde(rename = "control")]
    Control { seq_id: u64, action: String },

    /// Reload configuration
    #[serde(rename = "reload_config")]
    ReloadConfig { seq_id: u64 },
}

/// Response types sent from server to client
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcResponse {
    /// Response to selection request
    #[serde(rename = "selection_response")]
    SelectionResponse {
        seq_id: u64,
        index: i32,
        cancelled: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        child_index: Option<i32>,
    },

    /// Status response
    #[serde(rename = "status_response")]
    StatusResponse {
        seq_id: u64,
        listening: bool,
        paused: bool,
        active_profile: Option<String>,
    },

    /// Acknowledgment
    #[serde(rename = "ack")]
    Ack {
        seq_id: u64,
        success: bool,
        message: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_request_serialize() {
        let req = IpcRequest::SelectionRequest {
            seq_id: 1,
            title: "Select Artist".to_string(),
            items: vec!["Bach".to_string(), "Beethoven".to_string()],
            page: 1,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("selection_request"));
        assert!(json.contains("Bach"));
    }

    #[test]
    fn test_selection_response_serialize() {
        let resp = IpcResponse::SelectionResponse {
            seq_id: 1,
            index: 2,
            cancelled: false,
            child_index: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("selection_response"));
        assert!(json.contains("\"index\":2"));
    }
}
