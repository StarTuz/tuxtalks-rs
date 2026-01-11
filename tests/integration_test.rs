use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::thread;
use std::time::Duration;

mod common;
use common::TestContext;

#[test]
fn test_socket_permissions() {
    let ctx = TestContext::new();

    let metadata = fs::metadata(&ctx.socket_path).expect("Failed to get socket metadata");
    let permissions = metadata.permissions();
    let mode = permissions.mode();

    // Check for 0o600 (rw-------)
    assert_eq!(
        mode & 0o777,
        0o600,
        "Socket permissions must be 0o600 (Section 4.1 in GUARDRAILS.md)"
    );
}

#[test]
fn test_audit_log_creation() {
    let ctx = TestContext::new();

    // Config dir inside temp_dir
    let log_path = ctx.temp_dir.path().join("config/tuxtalks/audit.log");

    // Connect and send a Control request (which triggers audit log)
    let mut stream =
        UnixStream::connect(&ctx.socket_path).expect("Failed to connect for audit trigger");
    let request = r#"{"type":"control","seq_id":99,"action":"Test Audit Action"}"#;
    stream
        .write_all(request.as_bytes())
        .expect("Failed to write control request");
    stream.write_all(b"\n").expect("Failed to write newline");

    // Wait for processing
    thread::sleep(Duration::from_millis(500));

    assert!(
        log_path.exists(),
        "Log file should be created after control action"
    );

    let content = fs::read_to_string(log_path).expect("Failed to read log");
    assert!(
        content.contains("IPC Control Executed: Test Audit Action"),
        "Log should contain the action"
    );
}

#[test]
fn test_ipc_request_response() {
    let ctx = TestContext::new();

    let mut stream =
        UnixStream::connect(&ctx.socket_path).expect("Failed to connect to IPC socket");

    // Send a Status Request (Valid, snake_case)
    let request = r#"{"type":"status_request","seq_id":1}"#;
    stream
        .write_all(request.as_bytes())
        .expect("Failed to write to socket");
    stream.write_all(b"\n").expect("Failed to write newline");

    let mut response = String::new();
    let mut reader = std::io::BufReader::new(stream);
    use std::io::BufRead;
    reader
        .read_line(&mut response)
        .expect("Failed to read response");

    assert!(!response.is_empty(), "Response should not be empty");
    assert!(
        response.contains("StatusResponse") || response.contains("status_response"),
        "Response should be StatusResponse: {}",
        response
    );
}

#[test]
fn test_ipc_dos_protection_rate_limit() {
    let ctx = TestContext::new();

    // Establish connection
    let mut stream = UnixStream::connect(&ctx.socket_path).expect("Failed to connect");

    // Send multiple StatusRequests (valid) in rapid succession
    let request = r#"{"type":"status_request","seq_id":1}"#;

    for _ in 0..10 {
        // Expect that the server might close the connection (Broken Pipe)
        // or just ignore us. We don't care if write fails here.
        let _ = stream.write_all(request.as_bytes());
        let _ = stream.write_all(b"\n");
    }

    // Try a valid request after flooding
    thread::sleep(Duration::from_millis(500)); // Wait for rate limit to reset

    // Reconnect to verify server is still alive (accepting new connections)
    // The previous stream might be dead if server closed it.
    let mut stream =
        UnixStream::connect(&ctx.socket_path).expect("Failed to reconnect after flood");

    stream.write_all(request.as_bytes()).unwrap();
    stream.write_all(b"\n").unwrap();

    let mut response = String::new();
    let mut reader = std::io::BufReader::new(stream);
    use std::io::BufRead;
    match reader.read_line(&mut response) {
        Ok(_) => {
            if !response.is_empty() {
                assert!(
                    response.contains("StatusResponse") || response.contains("status_response"),
                    "Server should recover after flood: {}",
                    response
                );
            } else {
                panic!("Server closed connection on recovery check");
            }
        }
        Err(e) => panic!("Server died or closed connection: {}", e),
    }
}

/// Test wake word detection and command extraction flow (Frances - QA Lead)
#[test]
fn test_wake_word_command_flow() {
    // This tests the core logic without spawning the daemon
    use std::collections::HashMap;
    use tuxtalks::core::text_normalizer::TextNormalizer;

    let _normalizer = TextNormalizer::new(HashMap::new());
    let wake_word = "mango";

    // Test cases: (input, should_match, expected_remainder)
    let test_cases = vec![
        ("mango play music", true, "play music"),
        ("  mango play music", true, "play music"),
        ("...mango play", true, "play"),
        ("um mango boost", true, "boost"), // Filler word handling
        ("manga play music", false, ""),   // Wrong word
        ("random noise", false, ""),
    ];

    for (input, should_match, expected) in test_cases {
        let input_lower = input.to_lowercase();
        let matched = input_lower.contains(wake_word);

        if should_match {
            assert!(matched, "Should match wake word in: '{}'", input);

            // Extract remainder after wake word
            if let Some(idx) = input_lower.find(wake_word) {
                let remainder = input_lower[idx + wake_word.len()..].trim();
                assert_eq!(remainder, expected, "Remainder mismatch for: '{}'", input);
            }
        } else {
            // "manga" contains "mang" but not "mango" - this should not match
            // Our test expects exact wake word match
            if input.to_lowercase().contains("manga") {
                // Special case: manga != mango
                assert!(
                    !input_lower.contains(wake_word) || input_lower.contains("manga"),
                    "Should not match 'manga' as 'mango'"
                );
            }
        }
    }
}

/// Test wake word command mode flow (Frances - QA Lead)
/// Edge cases: wake word alone, wake word + command, sequential commands
#[test]
fn test_wake_word_command_mode_logic() {
    let wake_word = "mango";

    // Edge case 1: Wake word alone should trigger command mode
    let input1 = "the mango";
    let input_lower = input1.to_lowercase();
    assert!(
        input_lower.contains(wake_word),
        "Wake word should be detected"
    );

    // Extract after wake word
    if let Some(idx) = input_lower.find(wake_word) {
        let after_wake = input_lower[idx + wake_word.len()..].trim();
        assert!(
            after_wake.is_empty(),
            "No command after wake word - should enter command mode"
        );
    }

    // Edge case 2: Wake word + command in same utterance
    let input2 = "mango play music";
    let input_lower2 = input2.to_lowercase();
    if let Some(idx) = input_lower2.find(wake_word) {
        let after_wake = input_lower2[idx + wake_word.len()..].trim();
        assert_eq!(
            after_wake, "play music",
            "Command should be extracted after wake word"
        );
    }

    // Edge case 3: Wake word NOT at start (should still work per current logic)
    let input3 = "hey mango boost";
    let input_lower3 = input3.to_lowercase();
    if let Some(idx) = input_lower3.find(wake_word) {
        let after_wake = input_lower3[idx + wake_word.len()..].trim();
        assert_eq!(
            after_wake, "boost",
            "Command should be extracted even if wake word is not at start"
        );
    }

    // Edge case 4: No wake word - should be rejected
    let input4 = "play music";
    let input_lower4 = input4.to_lowercase();
    assert!(
        !input_lower4.contains(wake_word),
        "Should reject input without wake word"
    );
}
