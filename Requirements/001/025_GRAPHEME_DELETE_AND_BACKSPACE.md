# 025 — grapheme Delete and Backspace

Owner: CORE worker

Writable paths:
- `crates/editor-core/src/navigation.rs`
- `crates/editor-core/src/buffer.rs` only for delegating public API
- matching docs/tests

## Objective

Make ordinary forward/backward deletion delete one complete user-visible grapheme.

## Required implementation

1. Add/standardize `delete_forward_grapheme` and `delete_backward_grapheme` core operations.
2. Empty selection: use next/previous grapheme boundary.
3. Non-empty selection: delete selected range.
4. Multi-cursor edits are collected from pre-transaction coordinates and applied as one transaction.
5. Remove any need for root to calculate `current + 1`.
6. Preserve auto-pair special Backspace behavior where an empty auto-pair is deleted together; otherwise fall back to grapheme deletion.

## Required tests

- `K015_DELETE_GRAPHEME`
- `K016_BACKSPACE_GRAPHEME`
- combining accent
- multi-code-point emoji
- CRLF
- multi-cursor undo/redo

## Done only when

forward and backward delete cannot leave a partial grapheme and are fully undoable.
