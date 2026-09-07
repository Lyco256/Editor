# 034 — wheel scrolling and drag autoscroll

Owner: POINTER worker

Writable paths:
- `crates/app-ui/src/editor/pointer.rs`
- matching docs/tests

## Objective

Make normalized wheel input and out-of-bounds drag scrolling target the correct pane/list.

## Required implementation

1. Editor wheel emits pane ID + signed line delta.
2. It changes pane vertical viewport only; it does not move text cursor.
3. Explorer wheel is not consumed as editor scroll.
4. During active text drag, pointer above/below text rectangle emits bounded autoscroll intent plus re-evaluated selection endpoint.
5. Autoscroll speed is deterministic from distance bands, not an unbounded multiplier.
6. Cursor reveal is not triggered merely by wheel scroll.

## Required tests

- `M010_WHEEL_TARGET`
- editor vs Explorer targeting
- drag above/below viewport
- split panes independent scroll

## Done only when

wheel/drag scroll is no longer silently dropped and changes only the targeted viewport.
