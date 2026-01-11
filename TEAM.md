# TuxTalks Development Team

> These expert personas must be consulted on ALL changes.

## Core Team

### 🔧 Jaana Dogan - Head of Development / Systems Architect

**Focus:** Final architectural oversight, distributed systems, team expansion.
**Requirements:**

- **Final Sign-off**: All major architectural changes (Tier 2/3) require Jaana's explicit approval.
- **Team Expansion**: Authorized to recommend and vet new expert personas based on project needs.
- No `.unwrap()` or `.ok()` in production code.
- All async boundaries must be audited for blocking.
- Structured logging with tracing spans.
- Resource cleanup via Drop traits.
**Review triggers:** Any change to `mod.rs`, async code, error handling, or team structure.

### 🎤 Wendy Chisholm - Speech Systems Lead

**Focus:** ASR/TTS latency, accuracy, accessibility
**Requirements:**

- ASR confidence threshold (minimum 0.5)
- TTS mute window OR ASR pause during playback
- Model preloading where possible
- Phonetic matching for critical commands
**Review triggers:** Any change to `asr/`, `tts/`, wake word logic

### 🧠 Andrej Karpathy - AI Integration Specialist

**Focus:** Cortex (AI), LLM optimization, semantic intent resolution.
**Mandate:**

- Implement local embedding for long-term command context.
- Replace brittle regex/bypass logic with lightweight semantic intent matching.
- Optimize Ollama inference and context window management.
**Review triggers:** Any change to `processor.rs`, `cortex.rs`, or intent parsing logic.

### 🐧 Lennart Poettering - Systems & IPC Specialist

**Focus:** Hardening foundations, D-Bus, PipeWire, and multi-seat audio.
**Mandate:**

- Formalize Unix socket permissions and IPC security.
- Implement robust multi-seat audio routing and low-latency PipeWire streams.
- Ensure the D-Bus service is hardened against DoS and properly integrated with systemd/dbus-broker.
**Review triggers:** Any change to `ipc/`, audio routing, or system integration.

### 🛡️ Alex Stamos - Security Engineer (Red Team)

**Focus:** Input validation, command injection, audit logging, IPC security
**Requirements:**

- **Hardened IPC**: Unix sockets must use `0600` permissions.
- **DoS Protection**: IPC server must rate-limit incoming messages.
- **Disk-Backed Audit**: All executed actions must persist to `~/.config/tuxtalks/audit.log`.
- **Confirmation Prompts**: High-risk operations (e.g., self-destruct) MUST have voice confirmation.
**Review triggers:** Any change to `commands.rs`, `ipc/`, `games/`

### 🎨 Jony Ive - UX Design Lead

**Focus:** User feedback, error communication, visual states
**Requirements:**

- Visual indicator for listening state
- TTS feedback for rejected commands
- Toast notifications for errors
- Keyboard shortcuts for power users
**Review triggers:** Any change to `gui/`, user-facing messages

### 🧪 Frances Allen - Quality Engineering (Red Team)

**Focus:** Testing, failure analysis, reliability, edge cases
**Requirements:**

- **Fuzzing**: Test command processor with high-frequency/garbage ASR.
- **Failure Analysis**: Document behavior when Ollama/Piper/Vosk are unreachable.
- **Race Condition Guard**: IPC messages must use sequence IDs or nonces.
- **Integration Tests**: Mock traits for all external services.
**Review triggers:** Any PR without tests or error-handling paths

## Guardrails (Non-Negotiable)

All code MUST implement:

1. **ASR Confidence Gate** - Discard transcriptions < 0.5 confidence
2. **Rate Limiting** - Max 2 commands/second
3. **Error Logging** - No silent failures (`.ok()` → log warning)
4. **Validation** - Verify entities against data before action
5. **Audit Trail** - Log all executed commands

## Feature Roadmap (Team Consensus)

| Priority | Feature | Status |
| :--- | :--- | :--- |
| 🔴 HIGH | IPC (Unix socket) | Completed |
| 🟡 MED | Corrections UI | Planned |
| 🟡 MED | Voice Training | Planned |
| 🟢 LOW | Vocabulary Tab | Deferred |
| 🟢 LOW | speechd-ng | Deferred |

## Team Discussion: Dual Selection Strategy (CLI/Voice Fallback)

> [!NOTE]
> Following the implementation of GUI-based IPC selection, the team has agreed on a "CLI/Voice Fallback" for gaming and headless environments to avoid Alt-Tab requirements.

- **Wendy**: "Audio feedback is mandatory if the GUI is inactive. We must read the top choices."
- **Jony**: "The prompt must be concise. 'Select 1 for Artist X, 2 for Artist Y...'."
- **Jaana**: "The voice selection phase must be non-blocking and time-delimited (max 10s wait)."
- **Alex**: "Validate all voice/numeric inputs against the selection list indices to prevent errors."
- **Frances**: "Add a test case for 'IPC timeout fallback to voice'."
