# 030 — terminal native caret presentation

Owner: TERMINAL worker

Writable paths:
- `crates/terminal-backend/**`
- matching docs/tests

## Objective

Expose and implement a terminal-native steady vertical caret through the generic terminal adapter.

## Required implementation

1. Add `CursorPresentation` with hidden/visible, screen cell, and shape.
2. Support `SteadyBar` as required focused-editor shape.
3. Presentation order after framebuffer diff: move cursor -> set SteadyBar -> show cursor -> flush.
4. If requested cell is outside frame, hide cursor.
5. Menu/modal/non-editor focus can request hidden cursor.
6. Restore terminal cursor visibility/shape safely on exit.
7. Fake terminal records the last cursor presentation for tests.
8. Do not use a source framebuffer glyph as the primary caret.

## Required tests

- `M011_PRIMARY_CARET_PRESERVES_GLYPH`
- `M012_PRIMARY_CARET_STEADY_BAR`
- out-of-frame hide
- restore idempotency

## Done only when

generic runtime can present a SteadyBar independently from framebuffer text and terminal cleanup remains safe.
