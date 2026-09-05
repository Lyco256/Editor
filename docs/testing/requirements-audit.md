# Requirements audit (2026-09-05)

The integrated `devenv` and `main` refs point to the same verified commit.
Automated quality gates pass: formatting, workspace Clippy with warnings denied, all workspace
tests, source/document mirrors, release build, and redirected headless startup.

Implemented in the integration pass:

- persistent root `TextBuffer` editing, smart input, undo/redo, dirty-quit protection, atomic save
  effects, and deterministic save/reopen tests;
- application-data session snapshot/restore for all open tabs, active-tab selection, and unsaved contents;
- bounded search result backpressure, native `notify` watcher abstraction, typed replacement-range
  validation, typed LSP serialization errors, and panic terminal cleanup;
- root command palette plus Explorer/Output toggles;
- root workspace-root/Explorer projections with asynchronous refresh effects, mouse selection, PATH
  LSP discovery state, and asynchronous Git status effects;
- Windows installer (`build/editor.iss`) and packaging script (`build/package.ps1`).

Still release-blocking or environment-blocked:

1. Root orchestration does not yet connect every app-ui action to live LSP, syntax, workspace, and
   Git service state; the domain crates and snapshots are covered in isolation.
2. Root session persistence now restores all open tabs and active-tab selection; split-pane layout
   restoration and continuous recovery checkpoints are not yet wired.
3. Format-on-save/paste, system clipboard commands, mouse selection, project replace, and full
   Problems/Git/LSP views remain UI-service integration work rather than root effects (mouse
   selection and basic Git/LSP restart/status adapters are now covered).
4. Search and native watcher workers use standard threads; the architecture target is Tokio-based
   orchestration.
5. `docs/testing/performance.md` lacks resident-memory, idle-CPU, 10 MiB latency, and repeated
   open/close measurements because this environment cannot provide an interactive Windows Terminal.
   `docs/testing/final-smoke.md` records the same limitation. The master requirement requires user
   approval before treating those missing measurements as an exception.
6. Inno Setup (`iscc`) is not installed in this environment, so the installer definition has not
   been compiled; the release binary and staging package script are verified.

Accordingly, the repository is integrated and test-clean, but the product goal is not marked
complete until the remaining service wiring and Windows performance/smoke evidence are supplied.
