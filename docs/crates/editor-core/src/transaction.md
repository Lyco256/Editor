# Transaction boundary

Role: describes deterministic groups of simultaneous edits and optional resulting selection state.

Important types:

- `Edit` represents one replacement expressed in character offsets.
- `Transaction` stores ordered edits and an optional post-edit selection set.
- `TransactionBuilder` sorts edits, optionally deduplicates identical ones, and rejects overlaps.

Invariants:

- Edits are applied against the original input text in a deterministic order.
- Overlapping or colliding edits are rejected unless identical duplicates are explicitly normalized.
- A transaction can optionally carry the exact resulting selection set for smart editing paths.

Data flow:

- Higher-level commands build `Edit` values, pass them through `TransactionBuilder`, and then hand
  the resulting `Transaction` to `TextBuffer`.
- Undo history stores both forward and inverse transactions.

Concurrency and error behavior:

- The module is pure data construction and validation.
- Invalid edit ordering or overlap returns `EditorError::OverlappingEdits`.

Tests:

- Buffer tests cover multiple-edit ordering and exact undo/redo behavior.
