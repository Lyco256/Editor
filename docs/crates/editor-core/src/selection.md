# Selection boundary

Role: models anchor/active selections and normalized multi-selection sets for the text engine.

Important types:

- `Selection` stores one anchor and one active caret offset.
- `SelectionSet` stores one or more selections plus a distinguished primary selection index.

Invariants:

- A selection range is always normalized so `start <= end`.
- A `SelectionSet` cannot be empty.
- The primary selection index always points to one of the stored selections.
- Duplicate selections are removed during construction and mapping.

The selection set also exposes cursor composition helpers (`with_cursor`, `without_last_cursor`,
and `collapse`) so multi-cursor edits preserve a distinguished primary selection.

Data flow:

- Cursor and selection movement code builds new `Selection` values and then re-normalizes them
  through `SelectionSet::new`.
- Buffer mutation paths use the current set to transform selection state after edits.

Concurrency and error behavior:

- This module is purely in-memory data modeling.
- Construction returns typed errors for empty sets and invalid primary indices.

Tests:

- Buffer tests exercise selection transformation through undo/redo and movement.
