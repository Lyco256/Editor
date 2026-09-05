# Wire protocol boundary

Role: defines the small set of JSON-RPC and LSP wire types this crate needs, along with the framing
helpers for stdio transport.

Important types:

- `CommandSpec` describes a structured executable, argument vector, environment map, and working
  directory.
- `CommandSpec::discover_known` resolves conventional server names from `PATH` without executing
  them; root orchestration must still authorize and spawn the result.
- `InitializeParams`, `InitializeResponse`, and `ServerInfo` model the LSP handshake.
- `TextDocumentItem`, `DidOpenTextDocumentParams`, `DidChangeTextDocumentParams`, `DidSaveTextDocumentParams`,
  and `DidCloseTextDocumentParams` cover document notifications.
- `PublishDiagnosticsParams`, `Diagnostic`, `JsonRpcRequest`, `JsonRpcNotification`, and
  `JsonRpcResponse` represent the wire messages used by the client and fake server.
- `IncomingMessage` distinguishes requests, notifications, and responses after parsing.

Invariants:

- Frames are encoded with a valid Content-Length header and raw JSON payload.
- Malformed headers, malformed JSON, and unsupported response IDs become typed protocol errors.
- The negotiated position encoding defaults to UTF-16 when the server does not advertise a choice.

Data flow:

- `encode_message` and `write_message` serialize a JSON-RPC value into a stdio frame and return a typed protocol error if serialization fails.
- `read_message` parses headers and payload bytes back into a typed incoming message.
- The position helpers convert between LSP wire positions and editor logical positions using plain
  text and the negotiated encoding.

Concurrency and error behavior:

- The module is transport-only and performs no process management.
- Invalid frames never panic; they return `ProtocolError` or `MessageIoError`.

Tests:

- The crate integration tests cover round-trip position conversion and malformed-message handling.
