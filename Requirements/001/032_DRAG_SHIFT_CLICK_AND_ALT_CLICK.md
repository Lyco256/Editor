# 032 — drag Shift click and Alt click

Owner: POINTER worker

Writable paths:
- `crates/app-ui/src/editor/pointer.rs`
- matching docs/tests

## Objective

Implement selection gestures from typed pane-local pointer targets.

## Required implementation

1. left Down places primary caret and starts drag anchor;
2. left Drag extends primary selection with the same EOL/EOF clamp as task 031;
3. left Up ends drag;
4. Shift+left Down extends existing primary anchor to clicked position;
5. Alt+left Down emits add-secondary-cursor action at mapped position;
6. line-number click selects full logical line;
7. line-number drag extends whole-line selection;
8. fold-marker click emits fold toggle;
9. overview click emits reveal/scroll request, not a raw text caret placement.

## Required tests

- `M005_DRAG_CLAMP_EOL_EOF`
- `M006_SHIFT_CLICK`
- `M007_ALT_CLICK_CURSOR`
- line-number and fold/overview tests

## Done only when

gesture actions are pane-local, coordinate-safe, and preserve selection anchors.
