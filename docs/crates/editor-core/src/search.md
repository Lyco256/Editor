# In-document search boundary

Role: provides literal and regular-expression find/replace operations within one buffer.

Important types:

- `SearchKind` selects literal or regex interpretation.
- `FindOptions` configures case sensitivity and whole-word matching.
- `FindMatch` stores a character-offset range suitable for selections and overview markers.

Invariants:

- Empty queries are treated as no-op searches.
- Search results and replacements are expressed in character offsets, not byte offsets.
- Regex replacements remain one undoable transaction.

Data flow:

- `TextBuffer::find` compiles a `regex::Regex` from the caller options and maps byte matches back to
  character offsets.
- `TextBuffer::replace_all` expands regex captures when necessary and applies the replacements as a
  single transaction.

Concurrency and error behavior:

- The module is pure in-memory text processing.
- Regex compilation failures return `EditorError::InvalidRegex`.

Tests:

- Module tests cover literal and regex search, capture replacement, and invalid regex reporting.
