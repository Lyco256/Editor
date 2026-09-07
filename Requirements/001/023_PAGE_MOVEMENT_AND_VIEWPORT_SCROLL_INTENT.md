# 023 — Page movement and viewport scroll intent

Owner: CORE worker

Writable paths:
- `crates/editor-core/src/navigation.rs`
- matching docs/tests

## Objective

Implement PageUp/PageDown cursor movement and a separate Ctrl+Up/Down viewport-scroll intent.

## Required implementation

1. Page movement accepts current visible text-row count from caller.
2. PageUp/PageDown move approximately that many logical/visual rows in the current non-wrapped implementation.
3. Preserve preferred display column.
4. Shift extends selection.
5. Ctrl+Up/Down return a typed viewport-scroll intent and do not mutate text selection.
6. No editor-core method owns pane scroll offsets.

## Required tests

- `K011_PAGE_UP_DOWN`
- `K012_CTRL_UP_DOWN_SCROLL`
- page at document boundaries
- page through short lines while preserving goal

## Done only when

page motion and scroll-only motion are distinct, deterministic operations.
