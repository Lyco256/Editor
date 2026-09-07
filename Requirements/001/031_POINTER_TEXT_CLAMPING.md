# 031 — pointer text clamping

Owner: POINTER worker

Writable paths:
- `crates/app-ui/src/editor/pointer.rs`
- matching docs/tests

## Objective

Map pane-local pointer cells to useful valid document positions exactly, including whitespace outside text.

## Required implementation

The mapper accepts actual text subrect size, pane viewport, snapshot, and effective tab width.

Rules:
1. existing grapheme cell -> that grapheme boundary;
2. at/right of rendered EOL -> logical EOL;
3. existing blank line -> its EOL;
4. below last document line but inside text rect -> EOF;
5. left edge while horizontally scrolled -> nearest visible valid boundary;
6. never return None solely because pointer is to the right/below source content;
7. never target wide-character continuation or inside grapheme.

## Required tests

- `M001`–`M004`
- horizontally scrolled line
- tabs
- CJK
- combining grapheme

## Done only when

all clicks inside editor text resolve deterministically to valid document positions under the specified clamp rules.
