# 007 — branch and ownership contract

All worker branches start from the exact green foundation commit after requirement 015.

## Top-exclusive paths

Workers never modify:

- `src/app/**`
- `crates/editor-types/**`
- `crates/app-ui/src/shell/mod.rs`
- `crates/app-ui/src/shell/layout.rs`
- `crates/app-ui/src/shell/hit_test.rs`
- `crates/app-ui/src/editor/mod.rs`
- root `Cargo.toml`
- `Cargo.lock`
- frozen acceptance paths.

## Lane branches

- CORE: `feat/001-editor-core`
- TERMINAL: `feat/001-terminal`
- POINTER: `feat/001-editor-pointer`
- WORKBENCH: `feat/001-workbench`

A worker commits after each small task file and keeps its worktree clean.

The top Codex merges only after the lane's local crate tests, format, Clippy, and relevant frozen acceptance subset pass.
