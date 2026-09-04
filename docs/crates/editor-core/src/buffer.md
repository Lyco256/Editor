# Text buffer boundary

Role: implements the mutable Unicode text owner and the read-only snapshot API while keeping Ropey
hidden behind the crate boundary.

Important types:

- `TextBuffer` owns the rope, version counters, dirty tracking, undo/redo stacks, selections, and
  auto-pair markers.
- `TextSnapshot` exposes a stable `Arc<str>`-backed read view for consumers that need an immutable
  document image.
- `SemanticServicePolicy` tells higher layers whether semantic services are enabled or suppressed by
  large-file mode.
- `AppliedTransaction` reports whether a transaction changed text and which version it produced.

Invariants:

- A transaction updates the buffer version exactly once when it changes text.
- Undo restores the exact prior text, selections, and auto-pair marker state.
- Redo reapplies the reverted transaction exactly.
- UTF-8 byte offsets are never exposed as public editor positions.
- Caret boundaries reject CRLF split points.

Data flow:

- Public mutation methods (`insert`, `delete`, `replace`, and `apply_transaction`) validate edits
  against the current text, transform selections, and record inverse history.
- Navigation and coordinate conversion methods operate on logical character offsets and logical
  line/character positions, not display columns.
- Large-file detection is based on byte length and feeds the semantic-service suppression flag.

Concurrency and error behavior:

- The module is single-owner state and does not perform any external I/O.
- Invalid offsets, positions, ranges, overlapping edits, and malformed selections return typed
  `EditorError` values.

Tests:

- Module tests cover grapheme navigation, CRLF handling, undo/redo, overlapping edit rejection,
  and large-file suppression.
- Fixture-backed tests read from `tests/fixtures/editor-core` for CRLF and Unicode coverage.
