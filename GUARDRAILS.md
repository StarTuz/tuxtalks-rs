# TuxTalks Guardrails Requirements

> These are NON-NEGOTIABLE requirements. Code that violates these MUST NOT be merged.

## 1. Input Validation

### ASR Confidence

```rust
// REQUIRED: Reject low-confidence transcriptions
if confidence < 0.5 {
    warn!("🔇 Rejecting low-confidence transcription: {}", text);
    return;
}
```

### Rate Limiting

```rust
// REQUIRED: Max 2 commands per second
const MIN_COMMAND_INTERVAL_MS: u64 = 500;
if last_command_time.elapsed() < Duration::from_millis(MIN_COMMAND_INTERVAL_MS) {
    return; // Rate limited
}
```

## 2. Output Validation

### Entity Verification

```rust
// REQUIRED: Verify entities exist before action
if let Some(artist) = intent.parameters.get("artist") {
    if !library.artist_exists(artist) {
        speak("I couldn't find that artist").await;
        return;
    }
}
```

### Destructive Action Confirmation

```rust
// REQUIRED: High-risk game commands need confirmation
const DANGEROUS_COMMANDS: &[&str] = &["self destruct", "eject", "abandon"];
if DANGEROUS_COMMANDS.iter().any(|c| command.contains(c)) {
    speak("Are you sure? Say confirm to proceed").await;
    wait_for_confirmation().await?;
}
```

## 3. Error Handling

### No Silent Failures

```rust
// FORBIDDEN:
config.save().ok();

// REQUIRED:
if let Err(e) = config.save() {
    warn!("Failed to save config: {}", e);
}
```

## 4. Audit Logging

### Action Log

```rust
// REQUIRED: Log all executed commands
info!(
    action = "executed",
    command = %command_name,
    confidence = %confidence,
    source = %source,
    "🎯 Command executed"
);
```

## 5. Async Safety

### No Blocking in Async

```rust
// FORBIDDEN: Blocking calls in async context
let status = child.wait()?;

// REQUIRED: Use spawn_blocking
tokio::task::spawn_blocking(move || {
    child.wait()
}).await??;
```

## 6. AIAM: Agent Governance

### No-Touch Zones

- **FORBIDDEN:** Modifying or deleting system binaries (e.g., `/usr/bin/`, `/home/startux/.local/bin/`).
- **FORBIDDEN:** Modifying global configuration files outside the project workspace without explicit EV.
- **FORBIDDEN:** Deleting project source files based on heuristic assumptions.

### Action Risk Tiers

- **Tier 0 (Safe):** Read-only operations, linting, UI state changes.
- **Tier 1 (Normal):** Incremental code edits, adding new feature files.
- **Tier 2 (High Risk):** Dependency changes (`Cargo.toml`), database schema migrations, external API integration.
- **Tier 3 (Restricted):** File deletions, binary modifications, environment-wide configuration changes.

### Mandatory Verification (EV)

- **REQUIRED:** Before proposing Tier 2 or Tier 3 actions, the agent MUST use `view_file` or `ls -l` to verify the current state.
- **REQUIRED:** All Tier 3 actions must be logged with a `justification` in the audit log.
- **REQUIRED:** Agents must verify the `PlayerManager` state before applying player-specific logic.
