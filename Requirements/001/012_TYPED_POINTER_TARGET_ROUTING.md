# 012 — typed pointer target routing

Owner: top Codex

Writable paths:
- `src/app/pointer_input.rs`
- `src/app/runtime.rs`
- `crates/app-ui/src/shell/hit_test.rs`
- matching docs

## Objective

Remove all root screen-coordinate guesses and translate raw terminal mouse events through the stored layout snapshot.

## Required implementation

1. Define typed hit targets for menu, menu item, Explorer row, tab, editor line-number area, editor gutter/fold area, editor text area, overview ruler, split separator, bottom panel, status, overlay, and outside.
2. Editor targets carry exact pane ID and local coordinates relative to the relevant subrect.
3. Runtime/hit-test layer converts raw screen mouse event to typed target before `AppState` handles it.
4. Remove root checks equivalent to Explorer `<24/<25`, split `>=60/>=20`, global editor subtraction constants, tab `column / fixed`, and Git row thresholds independent of current panel rect.
5. A typed target is based on the last presented layout snapshot; if no snapshot exists, ignore pointer input rather than guessing.
6. Pointer events on a newly resized frame use the newly stored snapshot.

## Required tests

- `R004`–`R007`
- static/architecture tests for banned fixed-coordinate paths
- pointer target identity tests before/after Explorer toggle and bottom-panel toggle

## Done only when

root input code contains no global-workbench magic coordinates and every mouse action carries an exact rendered target.
