# Subagent requirement — editor-core

## Branch

`feat/editor-core`

## Writable ownership

- `crates/editor-core/**`
- `docs/crates/editor-core/**`
- editor-core-specific fixtures under `tests/fixtures/editor-core/**`

Do not edit root application files, shared types, other crates, or `Cargo.lock`.

## Scope

Implement the text editing engine with Ropey hidden behind the crate API.

Required components:

- `TextBuffer`,
- immutable/read-only text snapshot API for syntax and LSP,
- line index and conversions,
- transaction model,
- insert/delete/replace,
- multiple simultaneous edits,
- `SelectionSet`,
- multiple cursors,
- cursor movement,
- grapheme-safe deletion/navigation where user-visible text behavior requires it,
- undo/redo history,
- dirty/version tracking,
- bracket/pair metadata required for smart editing,
- smart pair insertion/overtype/deletion,
- smart Enter between paired delimiters,
- indentation primitives,
- find/replace within a document,
- code-fold region model,
- large-file mode flag and semantic-service suppression signal.

Ropey is private implementation detail.

## Invariants

- One transaction increments the document version once.
- Undo exactly reverses a transaction.
- Redo exactly reapplies the reverted transaction.
- Multi-edit operations are deterministic and do not shift later edit locations incorrectly.
- Invalid overlapping edits return a typed error unless the transaction builder explicitly normalizes them.
- Cursor/selection positions never point into an invalid internal character boundary.
- Logical positions are not display columns.
- CRLF handling does not corrupt logical lines.
- Saving-related newline/encoding transformations are not implemented here; this crate stays Unicode-text focused.

## Tests

Use unit, parameterized, and property tests.

Required property areas:

- arbitrary insert/delete sequences preserve valid text,
- transaction + undo returns exact original text and selections,
- undo + redo returns exact edited text and selections,
- multiple edit ordering,
- line/character conversion round trips,
- Unicode combining characters,
- emoji and wide characters,
- CRLF/LF text,
- auto-pair insertion/overtype/deletion,
- Enter expansion inside `{}`, `[]`, and language-configured pairs.

No test depends on a real terminal.

## Acceptance criteria

- All required editing operations are exposed through stable crate APIs.
- Ropey is not visible in public types.
- The crate does not perform filesystem, terminal, Git, or process I/O.
- Property tests cover the listed invariants.
- `cargo test -p editor-core` passes.
- `cargo clippy -p editor-core --all-targets -- -D warnings` passes.
- All owned source files have mirrored documentation.
