# 021 — arrow and selection semantics

Owner: CORE worker

Writable paths:
- `crates/editor-core/src/navigation.rs`
- matching docs/tests

## Objective

Match conventional/Edit Left/Right/Up/Down behavior, including selection collapse and Shift extension.

## Required implementation

1. Left/Right move by grapheme boundary.
2. Without Shift and with non-empty selection: Left -> selection start, Right -> selection end.
3. Shift+Left/Right preserves anchor and moves active end.
4. Up/Down use the preferred-column state from task 020.
5. Shift+Up/Down extends from anchor while preserving vertical goal.
6. Boundary movement at document start/end is a no-op with valid selection state.
7. No motion splits CRLF or a grapheme cluster.

## Required tests

- `K001`–`K007`
- boundary tests at start/end
- combining/emoji grapheme fixtures

## Done only when

arrow behavior matches the specified conventional semantics for single and multiple selections.
