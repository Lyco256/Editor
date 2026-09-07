# Subagent requirement — generic picker UX

## Branch

`feat/generic-picker-ux`

## Writable ownership

- `crates/app-ui/src/widgets/**`
- `docs/crates/app-ui/src/widgets/**`
- `tests/fixtures/app-ui/widgets/**`
- widget-specific snapshots under `tests/snapshots/**`

Do not edit root `src/app/**`, shared types, shell/editor/language/workspace/Git feature directories, or `Cargo.lock`.

## Goal

Provide one deterministic terminal picker used by the top integration stage for finite choices such as encodings, EOL modes, recent workspaces, and other selection lists.

## Required behavior

The picker supports:

- title,
- optional query,
- rows with stable typed/opaque IDs,
- row label,
- optional detail text,
- enabled/disabled state,
- selected row,
- incremental case-insensitive filtering,
- Up/Down,
- PageUp/PageDown,
- Home/End,
- Enter accept,
- Escape cancel,
- mouse selection and activation,
- scroll window for lists longer than available height.

Filtering never changes row identity.

Disabled rows cannot be accepted.

## Layout

The picker:

- is centered or predictably anchored within the available frame,
- never writes out of bounds,
- has a deterministic compact layout,
- keeps the selected row visible,
- shows query text when filtering is enabled.

## Tests

Snapshot/state tests cover:

- normal list,
- filtered list,
- disabled row,
- long scrolling list,
- mouse activation,
- compact terminal,
- Unicode labels,
- stable row identity after filtering.

## Acceptance criteria

- The picker is feature-neutral.
- It emits selected row identity without parsing rendered labels.
- It requires no platform GUI dialog.
- All owned tests, Clippy, formatting, and docs mirrors pass.
