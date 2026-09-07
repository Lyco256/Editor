# 020 — preferred display column state

Owner: CORE worker

Writable paths:
- `crates/editor-core/src/navigation.rs`
- minimal related `editor-core` types required by this behavior
- matching docs/tests

## Objective

Store a persistent vertical goal display column for each selection/cursor.

## Required implementation

1. Add per-selection navigation goal state or an equivalent index-stable structure owned by `TextBuffer`; it must support multiple cursors independently.
2. A first vertical move captures current display column using supplied tab width.
3. Consecutive vertical moves reuse the stored goal even after clamping.
4. Horizontal/direct placement/edit operations can explicitly reset the affected goal.
5. Selection normalization preserves the correct goal for surviving cursor identities.
6. Undo/redo restores navigation goal state when selection state is restored.

## Required tests

- `K004`–`K007`
- undo/redo preferred-column state
- tab widths 2,4,8
- CJK wide-character goal

## Done only when

short/empty intermediate lines never destroy the intended vertical display column for any cursor.
