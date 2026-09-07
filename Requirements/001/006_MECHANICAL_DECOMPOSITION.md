# 006 — mechanically decompose monolithic UI/input files

Owner: top Codex.
Run before behavior changes in parallel lanes.

## Purpose

Current `state.rs`, `shell/mod.rs`, and `editor/mod.rs` contain multiple ownership domains. Split them so worktree agents can modify disjoint files.

This task changes file organization only. Existing non-001 behavior tests must remain unchanged.

## Root app split

Keep `src/app/state.rs` as `AppState` data, high-level transitions, and module declarations.

Move ordinary editor keyboard handling into:

- `src/app/editor_input.rs`

Move raw/translated mouse application handling into:

- `src/app/pointer_input.rs`

Move scene projection helpers out of runtime into:

- `src/app/scene.rs`

`src/app/runtime.rs` keeps runtime lifecycle/event loop/presentation.

## Shell split

Make `crates/app-ui/src/shell/mod.rs` a facade and shared public types only.

Create:

- `shell/layout.rs`
- `shell/hit_test.rs`
- `shell/menu.rs`
- `shell/explorer.rs`
- `shell/tabs.rs`
- `shell/workbench.rs`
- `shell/panes.rs`
- `shell/status.rs`
- `shell/theme.rs`

Move code without changing rendered output or action semantics yet.

## Editor UI split

Make `crates/app-ui/src/editor/mod.rs` a facade/shared state only.

Create:

- `editor/viewport.rs`
- `editor/pointer.rs`
- `editor/cursor.rs`
- `editor/selection.rs`
- `editor/text_metrics.rs`

Move the existing viewport/cursor helpers mechanically.

## Editor core split

Create:

- `crates/editor-core/src/navigation.rs`

Move navigation helper algorithms there while preserving the existing public `TextBuffer` methods as delegating API until later tasks change behavior.

## Documentation

Create matching mirror docs for every new production Rust source file.

## Gate

Before and after decomposition:

- existing normal test results must match;
- no 001 acceptance case is edited;
- `cargo fmt` passes;
- Clippy passes for touched crates.

Commit:
`refactor(001): split editor interaction ownership`
