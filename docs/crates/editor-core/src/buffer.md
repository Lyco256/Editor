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
- `TextBuffer` also supports structural cloning/equality for root-state snapshots; Ropey remains
  private to this module.
- Grapheme navigation is exposed through `TextBuffer::next_grapheme_offset`, so forward deletion
  and cursor movement share the same Unicode-safe boundary logic.

Invariants:

- A transaction updates the buffer version exactly once when it changes text.
- Undo restores the exact prior text, selections, and auto-pair marker state.
- Redo reapplies the reverted transaction exactly.
- Recovered unsaved snapshots can be marked dirty without synthesizing an undoable edit.
- UTF-8 byte offsets are never exposed as public editor positions.
- Caret boundaries reject CRLF split points.

`TextSnapshot::from_text` is available for immutable compatibility projections and does not create
an editable buffer or reset edit history.

Cursor composition is Unicode-safe and transaction-compatible: `add_cursor_above/below`,
`remove_last_cursor`, `collapse_selections`, and occurrence selection APIs operate on character
offsets and retain undo/redo state.

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
