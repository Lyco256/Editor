# Smart editing boundary

Role: implements pair-aware insertion, overtype, paired deletion, smart Enter, and indentation
helpers on top of `TextBuffer`.

Important types:

- `IndentStyle` models tabs or a fixed number of spaces.
- `PairConfig` defines a language-aware opening/closing delimiter pair.
- `SmartEditOutcome` reports whether a command changed text or selection state and what version it
  produced.

Invariants:

- Smart insert and backspace stay grapheme-safe.
- Auto-inserted empty pairs can be overtyped and deleted as a pair when tracked.
- Smart Enter preserves the document newline convention and applies the requested indentation
  style.
- Indentation edits are grouped into one transaction.

Data flow:

- `smart_insert` inspects the current selections, applies pair expansion or overtype rules, and
  records auto-pair markers when needed.
- `smart_backspace` deletes the selection or the preceding grapheme, including tracked pairs.
- `smart_enter` expands configured pairs and inserts the appropriate indentation scaffold.
- `indent_selections` computes touched lines from a snapshot and indents them in one transaction.

Concurrency and error behavior:

- The module only manipulates in-memory text state.
- Invalid pair definitions and invalid selection/state combinations surface typed errors.

Tests:

- Module tests cover pair insertion, overtype, paired deletion, newline preservation, and
  configuration-driven Enter expansion.
