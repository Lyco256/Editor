# Position mapping boundary

Role: converts between `editor-core::LogicalPosition` values and negotiated LSP wire positions using
an immutable `TextSnapshot`.

Important types:

- `PositionMapper` holds a snapshot reference plus the active wire encoding.
- `PositionError` wraps protocol-level conversion failures.
- `PositionEncoding` selects UTF-8 or UTF-16 wire offsets.

Invariants:

- Position conversion always starts from a frozen snapshot, not the mutable buffer.
- UTF-8 positions are validated against scalar boundaries.
- UTF-16 positions reject offsets that split a surrogate pair.
- The mapper never treats byte, scalar, and code-unit counts as interchangeable.

Data flow:

- `to_lsp` converts an editor logical position into the negotiated wire encoding.
- `from_lsp` converts a wire position back to the editor logical coordinate system.

Concurrency and error behavior:

- The mapper is pure read-only logic and performs no I/O.
- Invalid positions return typed errors instead of panicking.

Tests:

- Integration tests cover ASCII, emoji, combining marks, and mixed UTF-8/UTF-16 line content.
