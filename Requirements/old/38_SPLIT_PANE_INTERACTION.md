# Subagent requirement — split pane interaction

## Branch

`feat/split-pane-interaction`

## Writable ownership

- `crates/app-ui/src/shell/**`
- `docs/crates/app-ui/src/shell/**`
- `tests/fixtures/app-ui/shell/**`
- shell-specific snapshots under `tests/snapshots/**`

Do not edit root `src/app/**`, `crates/app-ui/src/editor/**`, shared types, other feature UI directories, or `Cargo.lock`.

## Goal

Make split editors independently addressable and interactive instead of treating the whole editor area as one undifferentiated hit region.

## 1. Pane identity

Shell layout/render/hit-test APIs preserve a stable pane identifier supplied by root state.

An editor hit result contains the pane ID and local pane coordinates.

A tab strip hit remains distinct from an editor pane hit.

## 2. Pane focus

Shell state visibly distinguishes the focused pane.

Keyboard focus navigation supports:

- focus left pane,
- focus right pane,
- focus pane above,
- focus pane below.

If no pane exists in the requested direction, focus does not change.

Mouse click in a pane focuses that pane before the root applies cursor/selection behavior.

## 3. Split geometry

Horizontal and vertical split geometry remains deterministic.

The shell supports nested `PaneNode` layout, while the root may choose how many panes to create.

Split ratio is clamped so both child panes remain usable when terminal dimensions permit.

When the terminal is too small, existing collapse rules preserve the focused editor pane.

## 4. Pane-local coordinates

Hit testing returns coordinates relative to the pane's text area after gutter/line-number/overview allocation.

This allows root mouse logic to map screen positions to the correct document offset without assuming the primary pane.

## Tests

Framebuffer/layout tests cover:

- vertical two-pane focus,
- horizontal two-pane focus,
- nested split hit identity,
- directional focus,
- mouse pane selection,
- pane-local coordinate conversion,
- split-ratio clamping,
- compact terminal collapse preserving focused pane.

## Acceptance criteria

- `ShellHit::Editor` or its replacement identifies the exact pane.
- Directional keyboard focus is deterministic.
- Mouse hit testing identifies the same pane that was rendered at the coordinate.
- No filesystem/process/editor mutation occurs inside shell rendering.
- All owned tests, Clippy, formatting, and docs mirrors pass.
