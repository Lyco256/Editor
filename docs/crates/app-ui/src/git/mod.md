# `crates/app-ui/src/git/mod.rs`

This module owns the pure Source Control UI boundary for the terminal application.

It converts Git repository state, diff structures, branch and stash lists, log entries, and trust
policy into deterministic framebuffer output. It also defines the normalized UI actions and effect
requests that the shell can forward to the runtime.

Important types:

- `GitView` selects the current Source Control subview.
- `GitChangeMode` toggles working-tree versus index presentation.
- `GitTrustState` records whether Git actions are enabled.
- `GitDashboardState` is the main renderable model.
- `GitAction` describes user intent.
- `GitEffectRequest` describes typed work requests sent to the backend layer.
- `GitRepositorySnapshot` groups the repository inputs needed for tests and view construction.

Key invariants:

- The module does not execute Git or build shell commands.
- Destructive actions are represented explicitly and require confirmation state.
- Busy state, stderr, and trust gating remain visible in the UI.
- Rendering is deterministic and only depends on model inputs and terminal size.

Tests cover:

- clean repository presentation,
- mixed staged/unstaged/untracked/conflict states,
- working-tree/index transitions,
- diff and hunk rendering,
- commit form rendering,
- branch, stash, history, and conflict views,
- command failure output,
- untrusted workspace blocking,
- narrow layout behavior,
- typed action dispatch.
