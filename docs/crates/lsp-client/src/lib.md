# LSP client boundary

Role: owns the stdio Language Server Protocol client, the negotiated position-encoding bridge, and
the external formatter runner used when LSP formatting is unavailable.

Important types:

- `LspClient` manages process lifecycle, request/response correlation, cancellation, diagnostics,
  stderr capture, and restart state.
- `PositionMapper` bridges `editor-core::TextSnapshot` values to negotiated UTF-8 or UTF-16 wire
  positions.
- `FormatterRunner` executes an already-authorized formatter command and returns a single document
  replacement edit.
- `CommandSpec`, `InitializeParams`, `InitializeResponse`, `PublishDiagnosticsParams`, and the
  request/notification structs describe the wire-level data this crate sends and receives.

Invariants:

- Ropey remains hidden behind `editor-core`; this crate only uses `TextSnapshot`.
- LSP request IDs are unique within a client instance.
- Late responses for cancelled or superseded requests are ignored once the pending entry is removed.
- Malformed frames and invalid JSON become typed protocol errors instead of panics.

Data flow:

- The root layer constructs a structured `CommandSpec` and starts `LspClient`.
- `initialize` negotiates position encoding and capabilities, after which notification and request
  helpers send the remaining LSP traffic.
- `PositionMapper` uses a frozen snapshot plus the negotiated encoding to map editor positions to and
  from wire positions.
- `FormatterRunner` streams the snapshot text through the formatter process and returns a whole-file
  replacement edit on success.

Concurrency and error behavior:

- The crate uses Tokio for process I/O and request handling.
- Process output is captured on background tasks and surfaced as typed events.
- Cancellation and timeout failures are returned explicitly for formatter work.

Tests:

- Module and integration tests cover Unicode position conversion, lifecycle messages, diagnostics,
  completion, cancellation, malformed responses, crash recovery, and formatter success/failure
  paths.
- The repository doc-mirror test verifies that this file stays paired with `src/lib.rs`.
