# Requirements audit (2026-09-05)

`devenv` is the current integration branch at commit `1887f97`; it is not yet promoted to `main`
because release gates remain. Automated quality gates pass on `devenv`: formatting, workspace Clippy with warnings denied,
all workspace tests, source/document mirrors, release build, and redirected headless startup.

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
  population/status refresh after successful mutations; Tokio scheduling for root background work;
- root command-palette routing for Git Changes, Diff, Branches, Stashes, History, Commit, and
  Conflicts views, plus document-scoped `.editorconfig` indentation, encoding, EOL, whitespace,
  and final-newline behavior;
- confirmed root filesystem-operation prompts and background create/rename/move/delete effects,
  including dirty-buffer deletion protection and Explorer refresh;
- Windows installer (`build/editor.iss`) and packaging script (`build/package.ps1`).

Still release-blocking or environment-blocked:

1. `docs/testing/performance.md` now records release size, idle working-set/private memory, idle CPU,
   repeated redirected startup/close measurements, and a 21.22 ms deterministic 10 MiB edit
   transaction. Native keystroke-to-frame latency and repeated open/close of a 10 MiB document
   remain unavailable; `docs/testing/final-smoke.md` records the PTY smoke scope. The master
   requirement requires user approval before treating those missing measurements as an exception.
2. Root action coverage now includes every language-panel action and workspace file-operation
   confirmation path. The headless acceptance suite covers the fake-server initialize/request/response
   lifecycle, semantic tokens, Git projection/mutation routing, encoding round trips, and
   large-file suppression, as well as language panels, cancellation, crash notification, and
   replacement-server startup. A Syntax effect now reaches a rendered Root framebuffer. The
   `.editorconfig` root integration also has deterministic load and save-normalization tests.

Accordingly, the repository is integrated and test-clean, but the product goal is not marked
complete until the remaining performance/smoke evidence and promotion to an identical verified
`main` state are supplied.
