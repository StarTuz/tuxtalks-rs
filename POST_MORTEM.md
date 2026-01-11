# Incident Report: Startup Silence Bug

**Root Cause:**
A **wildcard match arm (`_ => { ... }`)** in the GUI's event loop (`src/gui/mod.rs:519`) silently swallowed the `SpeechdConnected` message. This prevented the application from initializing the TTS engine reference and triggering the "Ready" announcement.

**Contributing Factors:**

1. **Compiler Silence:** The wildcard match satisfied Rust's exhaustiveness check, preventing the compiler from flagging the missing handler as an error.
2. **Hidden Logs:** The wildcard handler only logged at the `DEBUG` level (`debug!("Unhandled message...")`). Since the application runs at `INFO` level by default, these logs were invisible during standard execution.
3. **Test Coverage Gap:** Our recent Phase 7 Integration Tests focused exclusively on the headless **Daemon** binary (`tuxtalks`) to ensure CI stability. The **GUI Launcher** (`tuxtalks-launcher`) was verified via manual passing but lacked automated e2e tests for its specific event loop wiring.

**Correction:**
We have removed the implicit reliance on the wildcard for this critical message and explicitly implemented the `SpeechdConnected` handler to:

- Store the TTS engine reference.
- Update the UI status.
- Trigger the "TuxTalks Ready" announcement.

**Prevention Strategy:**

- **Refactor Wildcards:** Avoid using catch-all wildcards in critical state machine loops where handling every message is expected.
- **Elevate "Unhandled" Logs:** Change `debug!` to `warn!` or `error!` for unhandled messages in strict state machines to make them visible during runtime dev.
- **GUI Testing:** Expand the integration test suite to include a "headless" run of the GUI logic (testing the `update` function directly) to verify state transitions.
