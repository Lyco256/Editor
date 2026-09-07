# 003 — current failure map

This file records concrete current implementation paths that the cycle must remove or replace.

The top Codex confirms these paths against the checked-out `devenv` before setup, and records any line-number drift in `docs/testing/requirements-001-preflight.md`.

## Runtime

`src/app/runtime.rs`

Current defects include:

- `AppRuntime::size` is initialized and used by `frame_for_state`, but the event loop does not update it when an input Resize action arrives.
- rendering still contains a fallback that can build a compatibility `TextSnapshot` from `state.active_text` instead of using the persistent tab buffer snapshot.
- Git marker projection can span `0..buffer.len_chars()` for a changed file.
- project-search projection reconstructs match location with `line_text.find(matched_text)`.
- secondary viewport projection reuses primary language/Git/status data and hard-coded pane assumptions.
- contextual LSP overlay x/y is derived directly from document line/character rather than actual pane screen geometry.

## State/input

`src/app/state.rs`

Current defects include:

- `InputEvent::Resize { .. } => Ok(())`.
- Home/End/PageUp/PageDown are recognized by keybinding parsing but ordinary editor dispatch falls through to `_ => Ok(())`.
- Up/Down call `move_vertical(..., 4)` with hard-coded tab width.
- plain forward Delete constructs an end offset as current character offset + 1.
- mouse region routing uses fixed geometry constants for Explorer, split halves, editor offsets, tabs, and Git rows.
- normalized wheel input is not handled as targeted pane/list scrolling.

## Shell

`crates/app-ui/src/shell/mod.rs`

Current defects include:

- `hit_test` computes a hard-coded 120x40 layout.
- `Resize` is discarded by shell dispatch.
- wheel events return no shell action.
- tab hit behavior does not use exact rendered tab rectangles.
- Explorer chrome uses geometric Unicode triangles.
- compact labels use `chars().count()` and a single-character ellipsis.
- status compaction uses a bullet separator.
- no required top menubar is present.

## Editor viewport

`crates/app-ui/src/editor/mod.rs`

Current defects include:

- `render_cursors` writes a `▌` glyph over the source cell.
- text painting uses character-oriented width paths that require consolidation around grapheme/display-cell metrics.
- selection foreground/background roles are not sufficiently separated for readable conventional selection behavior.

## Editor core

`crates/editor-core/src/buffer.rs`

Current defect:

- `move_vertical` computes `desired` from the current active position on every vertical move, so a short/empty intermediate line destroys the prior goal column.

## Rule

Do not patch these failures by adding another parallel fallback path.

The old defective path is removed once its replacement is integrated.
