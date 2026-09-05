# Requirements audit (2026-09-05)

`devenv` is the current integration branch (the verified state is the tip of `devenv`); it is not yet promoted to `main`
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
- multi-root trust decisions now require and persist an explicit state for every canonical root;
- adding an untrusted root revokes aggregate trust and stops any running language server;
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
- direct terminal paste now participates in format-on-paste precedence and remains undoable;
- root `FindInDocument`/`ReplaceInDocument` actions now expose in-file search and undoable
  replace-all semantics through editor-core, with keyboard prompts and typed invalid-expression
  errors;
- root keyboard prompts now provide Quick Open, project search, and find/replace entry points;
  resolved VS Code keybindings dispatch through the same command path;
- persistent LSP `didChange` notifications now send the smallest changed range derivable from the
  previous/current snapshots;
- workspace-local and static-extension language configuration bracket pairs are applied to smart
  editing, while configuration failures remain visible as compatibility output;
- root headless acceptance coverage for create, rename, move, and delete confirmation workflows;
- Windows installer (`build/editor.iss`) and packaging script (`build/package.ps1`).

Release-gate status and evidence:

1. `docs/testing/performance.md` now records release size, idle working-set/private memory, idle CPU,
   repeated redirected startup/close measurements, a process-level five-iteration Windows Terminal
   10 MiB open/force-close sample, and a 21.22 ms deterministic 10 MiB edit transaction. Native
   keystroke-to-frame latency and clean interactive open/close memory behavior remain unavailable;
   `docs/testing/final-smoke.md` records the PTY smoke scope. The master requirement requires user
   approval before treating those missing measurements as an exception.
2. Root action coverage now includes every language-panel action and workspace file-operation
   confirmation path. The headless acceptance suite covers the fake-server initialize/request/response
   lifecycle, semantic tokens, Git projection/mutation routing, encoding round trips, and
   large-file suppression, as well as language panels, cancellation, crash notification, and
   replacement-server startup. A Syntax effect now reaches a rendered Root framebuffer. The
   `.editorconfig` root integration also has deterministic load and save-normalization tests.

3. Server-originated `workspace/applyEdit` requests now pass through root validation, a background
   atomic-save worker, open-tab refresh, and a typed JSON-RPC response. Multi-document edits,
   dirty-buffer rejection, and the fake-server response path have deterministic tests.
4. Root language commands now expose completion resolve, prepare rename, declaration/implementation
   navigation, range formatting, and syntax-aware structural selection expansion.

5. Trusted language-server requests now reuse the persistent client when available, and root queues
   didOpen/didChange/didSave/didClose plus workspace-folder notifications. File resource
   WorkspaceEdit operations (create/rename/delete) are validated, journaled, and applied with
   rollback on operation failure; directory resource operations remain explicitly rejected.

6. Client-originated rename/code-action WorkspaceEdits now use the background atomic worker when
   they target unopened documents. Remaining limitation is explicit rejection of directory
   resource operations; file resource operations and all text-document edits are handled.

7. Revoking Workspace Trust now stops persistent language-server sessions and rejects late server
   workspace edits, with deterministic root-state coverage.

Remaining product-integration gaps no longer include static VSIX archive discovery: runtime startup
now scans both unpacked extension directories and `.vsix` archives, extracts archives through the
path-safe compatibility boundary into an OS cache, and loads supported themes/snippets/language
configuration without executing extension code.
Explorer directory rows now expand/collapse through a background refresh; keyboard focus and
context-menu hit-testing remain outside the current shell surface. Language configuration now
drives line/block comments, word selection, bracket/surrounding pairs, indentation regexes, and
safe on-enter append/remove actions. Folding-marker-specific behavior remains outside the root
editor path. VS Code keybinding `when` contexts and two-stroke chords are
evaluated for the supported predicates; more complex context expressions remain unsupported.
Directory resource operations now support recursive rename/delete with in-memory rollback
journaling; non-recursive directory deletes return a typed error. The product goal is not marked complete
until these product gaps are resolved or explicitly accepted, the performance/smoke evidence is
supplied, and the identical verified state is promoted to `main`.
