# 029 — cursor reveal intent

Owner: CORE worker

Writable paths:
- `crates/editor-core/src/navigation.rs`
- matching docs/tests

## Objective

Expose enough result information for the root pane to reveal the active cursor after keyboard motion/editing without editor-core owning viewport state.

## Required implementation

1. Cursor-moving/editing operations return or make available the resulting active positions.
2. Define a typed `RevealCursor` intent/request that identifies primary active position and preferred minimal-reveal behavior.
3. Wheel-scroll-only operations do not emit immediate cursor reveal.
4. Keyboard cursor movement, typing, delete, Home/End, page navigation, and line operations request reveal.
5. Root later maps the position through actual pane geometry.

## Required tests

- movement emits reveal
- wheel-scroll intent does not
- multi-cursor uses primary cursor for automatic reveal

## Done only when

root can implement Microsoft Edit-like minimal cursor reveal without recomputing editor-core movement.
