# Requirements audit (2026-09-05)

The integrated `devenv` and `main` refs point to the same verified commit.
Automated quality gates pass: formatting, workspace Clippy with warnings denied, all workspace
tests, source/document mirrors, release build, and redirected headless startup.

Implemented in the integration pass:

- persistent root `TextBuffer` editing, smart input, undo/redo, dirty-quit protection, atomic save
  effects, and deterministic save/reopen tests;
- application-data session snapshot/restore for all open tabs, active-tab selection, split layout,
  tab order, multiple selections, missing-file recovery, and unsaved contents;
- root clipboard copy/cut/paste effects (with cut-after-write safety), an authorized external
  formatter effect, and recovery checkpoints after each action;
- canonical-path trust-store loading and saving at process bootstrap;
- bounded search result backpressure, native `notify` watcher abstraction, typed replacement-range
  validation, typed LSP serialization errors, and panic terminal cleanup;
- root command palette plus Explorer/Output toggles;
- root workspace-root/Explorer projections with asynchronous refresh effects, mouse selection, PATH
  LSP discovery state, and asynchronous Git status effects;
- root syntax refresh/folding/error markers, streaming project-search events with cancellation,
  structured trust-gated LSP request effects with negotiated position encoding, and Git dashboard
  population/status refresh after successful mutations;
- Windows installer (`build/editor.iss`) and packaging script (`build/package.ps1`).

Still release-blocking or environment-blocked:

1. Root still does not project all app-ui editor/language/workspace/Git models into rendered views:
   the runtime leaves syntax/semantic highlight spans, bracket matches, search-match markers, and
   several panel models unpopulated, and it does not complete every LSP result
   (completion/hover/rename/code actions/etc.). Syntax parser refresh, search streaming,
   diagnostics, and generic request effects now have root paths.
2. Format-on-save/paste root chaining exists behind explicit state flags, but settings/UI controls,
   LSP-formatting precedence, and full Problems/Git/LSP interactive views remain incomplete.
3. Root retains detected encoding/BOM/line-ending metadata in tabs, session records, status
   rendering, and save effects; explicit user conversion/reopen commands are not yet wired.
4. Search and native watcher workers use standard threads; the architecture target is Tokio-based
   orchestration.
5. `docs/testing/performance.md` lacks resident-memory, idle-CPU, 10 MiB latency, and repeated
   open/close measurements because this environment cannot provide an interactive Windows Terminal.
   `docs/testing/final-smoke.md` records the same limitation. The master requirement requires user
   approval before treating those missing measurements as an exception.
6. Inno Setup (`iscc`) is not installed in this environment, so the installer definition has not
   been compiled; the release binary and staging package script are verified.
7. The required root-level headless acceptance coverage is incomplete: `tests/headless_runtime.rs`
   does not yet exercise Syntax output, the fake-server LSP request lifecycle, Git dashboard
   mutations, or large-file semantic-service suppression end to end. Crate-level tests cover parts
   of these behaviors, but they do not prove the integrated root event/effect path.

Accordingly, the repository is integrated and test-clean, but the product goal is not marked
complete until the remaining service wiring, root acceptance coverage, and Windows performance/smoke
evidence are supplied.
