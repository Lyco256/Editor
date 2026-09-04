# Editor core boundary

Role: owns the document-editing engine that sits between protocol-facing consumers and Ropey.
`TextBuffer` is the mutable owner, `TextSnapshot` is the stable read view, and all other modules
support those two entry points.

Important types:

- `TextBuffer` stores text, selections, undo/redo history, auto-pair markers, large-file policy,
  and the current document version.
- `TextSnapshot` is an immutable `Arc<str>`-backed view for syntax and language clients.
- `Selection`, `SelectionSet`, `Transaction`, `Edit`, `FoldRegion`, `FoldSet`, `PairConfig`,
  `IndentStyle`, and search types are re-exported for crate users.
- `EditorError` captures typed invalid-input failures such as bad offsets, invalid ranges,
  overlapping edits, malformed pairs, and invalid fold regions.

Invariants:

- Ropey stays private to the crate implementation.
- Character offsets, logical positions, and display columns remain distinct concepts.
- One successful transaction increments the document version once.
- Undo and redo operate on transactions, not low-level writes.
- Large-file policy is derived from byte length and suppresses semantic services when active.

Data flow:

- Callers build `Transaction` values or use the higher-level smart-edit helpers on `TextBuffer`.
- `TextBuffer` validates offsets and ranges against its current rope, applies edits, updates
  selections, records history, and emits version/state metadata.
- `TextSnapshot` converts between offsets and positions without allowing future mutations to leak
  into the read view.

Concurrency and error behavior:

- The crate is pure text manipulation and performs no filesystem, terminal, Git, or process I/O.
- All expected invalid-input cases return `EditorError`; the implementation avoids hidden panics in
  owned production paths.

Tests:

- Unit tests in the module files cover line conversions, grapheme-safe navigation, transaction
  semantics, search, folding, and smart pair behavior.
- The repository doc-mirror test verifies that this file stays paired with `src/lib.rs`.
