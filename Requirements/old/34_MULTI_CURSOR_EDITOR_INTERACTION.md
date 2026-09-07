# Subagent requirement — multi-cursor editor interaction

## Branch

`feat/editor-multicursor`

## Writable ownership

- `crates/editor-core/**`
- `docs/crates/editor-core/**`
- editor-core-specific fixtures under `tests/fixtures/editor-core/**`

Do not edit root `src/app/**`, app-ui directories, shared types, or `Cargo.lock`.

## Goal

Expose complete deterministic editor-core operations required for a user to create and edit with multiple cursors, not merely restore multiple selections from session data.

## Required editor-core operations

Add stable operations for:

- add cursor above each current cursor,
- add cursor below each current cursor,
- add selection/cursor at an explicit valid document offset,
- remove the most recently added secondary cursor,
- collapse to primary cursor only,
- select the next occurrence of the primary selected text,
- skip the current occurrence and select the following occurrence,
- select all occurrences of the primary selected text,
- normalize duplicate cursors/selections,
- preserve primary-selection identity after normalization.

## Editing semantics

Typing, paste, delete, Backspace, auto-pair insertion, smart Enter, indentation, line-comment toggling, and snippet insertion must apply to every active selection/cursor as one transaction where the operation is supported.

A multi-cursor transaction:

- is deterministic regardless of cursor order,
- applies edits from stable pre-transaction coordinates,
- increments document version once,
- is undone in one Undo,
- restores all pre-transaction selections on Undo,
- restores all post-transaction selections on Redo.

## Vertical cursor behavior

Add-cursor-above/below preserves preferred display column where possible.

When a target line is shorter, the cursor clamps to the nearest valid position without splitting a grapheme cluster.

Tabs and wide characters use the editor's existing logical/display-coordinate conversion rather than byte offsets.

## Find-occurrence behavior

If the primary selection is non-empty, next-occurrence commands use that selected text.

If the primary selection is empty, the core selects the current word using the effective word-boundary rules supplied by the caller.

Search is non-overlapping by default and wraps once through the document.

Duplicate ranges are not added.

## Tests

Use unit and property tests covering:

- add cursor above/below across unequal line lengths,
- duplicate normalization,
- multiple insertion,
- multiple deletion,
- multi-cursor paired braces,
- multi-cursor smart Enter,
- multi-cursor paste,
- multi-cursor undo/redo,
- next occurrence and wrap,
- select all occurrences,
- emoji, combining characters, tabs, and wide characters,
- no invalid offsets after arbitrary supported multi-cursor edit sequences.

## Acceptance criteria

- Editor core exposes every required operation without terminal or UI dependencies.
- All multi-cursor edits are transactionally undoable.
- No operation creates duplicate or invalid cursor positions after normalization.
- `cargo test -p editor-core` passes.
- Clippy with denied warnings passes.
- Source documentation mirrors are complete.
