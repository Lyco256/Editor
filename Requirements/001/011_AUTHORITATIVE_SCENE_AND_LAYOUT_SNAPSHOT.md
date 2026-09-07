# 011 — authoritative scene and layout snapshot

Owner: top Codex

Writable paths:
- `src/app/scene.rs`
- `src/app/runtime.rs`
- `crates/app-ui/src/shell/layout.rs`
- `crates/app-ui/src/shell/hit_test.rs`
- matching docs

## Objective

Create one render result that contains the frame and the exact geometry used to produce it; this becomes the only source for pointer targeting.

## Required implementation

1. Define `WorkbenchLayoutSnapshot` in `shell/layout.rs`.
2. It stores full frame rect; menu; Explorer; bottom panel; status; every editor-group rect; each group's tab strip; each exact tab rect and tab identity; line-number/gutter/text/overview subrects; split separators; visible Explorer row rects and entry identity; overlay/menu popup rects when present.
3. Define a scene result in `src/app/scene.rs` containing `Framebuffer`, `WorkbenchLayoutSnapshot`, and optional terminal cursor presentation.
4. Make scene construction call layout once and render from that layout; do not call `shell.layout(...)` a second time later for input or overlays.
5. `AppRuntime` stores the layout snapshot only after successfully presenting the corresponding frame.
6. Every rect is clipped/contained in the frame.
7. Delete `ShellState::hit_test` logic that creates `Rect::new(0,0,120,40)`.
8. `shell/hit_test.rs` accepts an existing `&WorkbenchLayoutSnapshot` and a screen point; it never recalculates layout.

## Required tests

- `R003_STATUS_BOTTOM_ROW`
- `R004_RENDER_HIT_SAME_LAYOUT`
- `R005_NO_STALE_GEOMETRY`
- `R006_EXACT_TAB_HIT`
- `R007_SPLIT_EXACT_HIT`
- property tests for rect containment

## Done only when

a presented frame and its hit-test geometry are the same immutable layout product; no 120x40 hit-test substitute remains.
