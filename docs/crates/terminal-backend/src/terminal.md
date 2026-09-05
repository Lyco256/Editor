# Crossterm terminal lifecycle

Role: owns the runtime terminal mode transitions, cursor controls, and integration between input,
rendering, and cleanup.

Important types:

- `CrosstermBackend` combines lifecycle, input, and rendering for production use.
- `CursorShape` exposes portable cursor shape choices without leaking crossterm types.
- `TerminalError` classifies terminal I/O failures and inactive-state misuse.

Invariants:

- `enter` must happen before render or input operations.
- Cleanup is idempotent and best-effort.
- A failed `enter` still triggers restoration attempts for any modes that were already enabled.
- Drop performs a final restore attempt so terminal state is repaired during normal shutdown.

Data flow:

- `enter` enables raw mode and any supported alternate screen, mouse, paste, and keyboard flags.
- `render` delegates to the differential renderer.
- `read_input` and `poll_input` delegate to crossterm and normalize events through the input module.
- `restore` unwinds the enabled modes in reverse-appropriate order and clears the cleanup state.

Error behavior:

- All terminal operations return typed errors.
- Restore failure is reported without suppressing earlier failures, and cleanup remains retryable.

Dependencies:

- `crossterm` for terminal primitives.
- The local renderer, input, capability, and framebuffer modules.
- `editor-types` for terminal capability metadata and normalized input events.

Tests:

- The cleanup state machine is ordered and idempotent.
- Failed cleanup actions remain retryable until they succeed.
