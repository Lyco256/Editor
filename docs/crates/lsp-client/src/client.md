# Client process boundary

Role: owns the live LSP subprocess, pending-request table, event stream, and lifecycle transitions
for initialize, shutdown, restart, crash, and exit.

Important types:

- `LspClient` is the public handle used to send notifications, issue requests, and observe events.
- `ClientStatus` summarizes whether the server is starting, running, stopping, stopped, or crashed.
- `NegotiatedCapabilities` records which LSP features the server advertised during initialize.
- `ClientEvent` carries diagnostics, stderr lines, server requests, protocol failures, and exit
  notifications.
- `RequestTicket` represents a pending response that can be awaited or cancelled.

Invariants:

- One pending response entry exists per request ID.
- Cancelling a request removes its pending entry before late responses are processed.
- The client never blocks the caller while waiting for a server response; the wait happens on the
  returned ticket.
- Restarting fails outstanding pending requests from the previous generation before spawning the new
  process state.

Data flow:

- `spawn` starts the process with structured arguments and creates the reader tasks.
- `initialize` parses the server capabilities response and stores the negotiated encoding.
- Notification helpers serialize typed LSP params and write them through the process stdin.
- The stdout reader decodes messages, resolves pending requests, and forwards diagnostics or server
  requests through the broadcast channel.
- The stderr reader forwards diagnostic text to the event stream for Output visibility.

Concurrency and error behavior:

- Tokio mutexes guard process state and the pending map.
- Malformed frames, invalid responses, and server crashes become typed client errors and event
  notifications.
- Late responses for removed request IDs are ignored.

Tests:

- Integration tests exercise initialize, didOpen, incremental didChange, diagnostics, completion,
  rename, formatting, semantic tokens, inlay hints, cancellation, malformed response handling, and
  crash recovery against the fake server fixture.
