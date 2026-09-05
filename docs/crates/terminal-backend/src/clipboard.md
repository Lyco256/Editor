# Clipboard adapters

Role: provides a typed clipboard port for copy, cut, and paste commands.

Important types:

- `Clipboard` defines the text-only read and write boundary.
- `ClipboardError` classifies unavailable, read, and write failures without exposing clipboard content.
- `SystemClipboard` wraps the platform clipboard service through `arboard`.
- `MemoryClipboard` is a deterministic fake for tests and headless command handling.

Invariants:

- Clipboard reads and writes are text only.
- Cut operations must only remove buffer text after the clipboard write succeeds.
- Error messages are descriptive but never include clipboard contents.

Data flow:

- Higher layers call `write_text` for copy and cut paths.
- Paste paths call `read_text` and insert the returned string into the document model.
- Tests use `MemoryClipboard` so clipboard behavior remains deterministic.

Error behavior:

- `SystemClipboard::new` returns an unavailable error when the host session has no usable provider.
- Read and write errors remain typed so UI code can report them safely.

Dependencies:

- `arboard` for the host clipboard integration.
- `thiserror` for structured error messages.

Tests:

- The fake clipboard round-trips text.
- The unavailable fake provider reports an explicit error state.
