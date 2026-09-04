# Architecture summary for the owner

## Chosen architecture

Primary pattern: **unidirectional event-driven architecture + ports/adapters**, not MVVM.

Reasoning:

- A terminal UI is naturally driven by input/events and full-frame state rendering.
- LSP, Git, search, syntax parsing, filesystem work, and recovery are asynchronous effects, not view-model responsibilities.
- One authoritative state-update path prevents concurrent workers from mutating UI state.
- A headless framebuffer makes UI snapshots deterministic.
- Feature crates provide clean ownership boundaries for parallel Codex worktrees.

## Work split

### Top Codex before subagents

Stage A owns repository structure, Git rules, Cargo workspace, test tooling, docs mirror enforcement, and all empty feature modules.

Stage B owns shared types, app actions/events/effects, root runtime skeleton, feature registration, and fake/headless integration boundaries.

The top agent stops feature implementation at the point where contracts are frozen and the whole skeleton is green.

### Wave 1 — parallel backend/domain work

- editor-core
- terminal-backend
- config/encoding/recovery
- workspace/search/trust
- Tree-sitter syntax
- LSP client
- Git backend
- VS Code static compatibility

These branches own separate crates, so merge overlap is minimal.

### Wave 2 — parallel UI work

- shell/editor
- workspace UI
- language UI
- Git UI

These branches share the `app-ui` crate but own disjoint subdirectories. The central `lib.rs` and registrations are created in foundation and remain top-owned.

### Final top integration

The top Codex alone wires the root runtime, resolves interface mismatches, regenerates `Cargo.lock`, runs full tests, fixes integration defects, verifies performance/docs, and promotes `devenv` to `main`.

## High-conflict files reserved for top Codex

- root `Cargo.toml`
- `Cargo.lock`
- `rust-toolchain.toml`
- root `src/app/**`
- `crates/editor-types/**`
- `crates/app-ui/src/lib.rs`
- root test/verification orchestration
- shared module registration files

This ownership is the main mechanism used to prevent parallel-agent merge conflicts.
