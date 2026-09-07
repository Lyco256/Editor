# 022 — Home and End semantics

Owner: CORE worker

Writable paths:
- `crates/editor-core/src/navigation.rs`
- matching docs/tests

## Objective

Implement deterministic smart Home and line/document End semantics based on Microsoft Edit/VS Code expectations.

## Required implementation

1. Home on a logical line moves to first non-whitespace position.
2. If already at first non-whitespace, Home moves to absolute line start.
3. Repeated Home toggles between those positions.
4. End moves to logical line content end before line ending.
5. Ctrl+Home -> document start.
6. Ctrl+End -> document end.
7. Shift variants extend selection using original anchor.
8. These operations reset/recompute preferred vertical goal to the resulting display column.

## Required tests

- `K008`–`K010`
- all-whitespace line
- empty line
- first/last document line
- CRLF

## Done only when

Home/End and Ctrl variants behave exactly as specified and no longer depend on UI code.
