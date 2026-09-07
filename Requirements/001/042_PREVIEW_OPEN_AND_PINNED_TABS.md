# 042 — Preview Open and Pinned tabs

Owner: WORKBENCH worker

Writable paths:
- `crates/app-ui/src/shell/tabs.rs`
- matching docs/tests

## Objective

Represent and render VS Code-like temporary preview vs permanent/pinned editor states exactly.

## Required implementation

Tab disposition supplied by root is:
- Preview
- Open
- Pinned.

ASCII visual convention:
- Preview `<name>`
- Open `name`
- Pinned `^name`
- dirty prefix `*`
- visible close target `x`.

Rendering records exact tab/close rectangles.

No `chars().count()` for width; use shared display-cell metrics.

Overflow uses deterministic scrolling/clipping with ASCII navigation controls.

Pointer hit uses stored tab identity, never fixed division.

## Required tests

- `U006_PREVIEW_REPLACE`
- `U007_PREVIEW_PROMOTE_DIRTY`
- `U008_OPEN_PERMANENT`
- `U009_PIN_STATE`
- `R006_EXACT_TAB_HIT`
- CJK filename/tab width

## Done only when

every tab's disposition and clickable bounds are visually and structurally explicit.
