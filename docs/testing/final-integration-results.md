# Final integration results

Wave 2 integration is verified on the current tip of `devenv` (2026-09-07), and the same tested
commit is promoted to `main`. On this
verified integration state, format check, workspace Clippy with
warnings denied, all workspace tests, source/document mirror check, release build, and redirected
headless startup all pass through `tools/verify.ps1`. Root integration now covers persistent editing,
atomic save effects, dirty-quit protection, Save As, split/session recovery, clipboard transactions,
manual formatter transactions, project replacement effects, command palette routing for lifecycle and
high-level Git operations, shared encoding/EOL pickers, native watcher
abstraction, and session recovery.

The release binary was launched without arguments and with a file path in the current environment;
both exited 0 through the safe headless startup path. Five redirected 10 MiB file open/close runs
averaged 337.32 ms with a 35.10 MiB average peak working set. A PTY-backed smoke run, idle
memory/CPU samples, a five-iteration Windows Terminal 10 MiB process-level open/force-close sample,
and successful Inno Setup compilation are recorded in the smoke/performance documents; native
10 MiB typing-latency and clean interactive open/close profiling remain outstanding.

Server-originated `workspace/applyEdit` requests now pass through root validation, a background
atomic-save worker, open-tab refresh, and a typed JSON-RPC response. Multi-document edits and
dirty-buffer rejection are covered by deterministic tests. Root commands also expose completion
resolve, prepare rename, declaration/implementation navigation, range formatting, and
syntax-aware structural selection expansion.

Trusted language-server requests now reuse the persistent client session when available. Root
queues workspace-folder, didOpen, didChange, didSave, and didClose notifications; deterministic
state coverage verifies the lifecycle notifications are emitted. Directory resource operations
support recursive rename/delete with in-memory rollback journaling; non-recursive directory deletes
return a typed error.
In-document FindInDocument/ReplaceInDocument actions now expose editor-core search and undoable
replace-all behavior through root state, with keyboard prompt entry and typed invalid-expression
errors. Quick Open and project search also have keyboard prompt entry, and resolved VS Code
keybindings dispatch through root commands.
Explorer directory clicks now toggle an expanded-directory set and dispatch background refreshes;
Ctrl+Shift+E focuses the tree for keyboard navigation and Enter opens files; file rows continue to
open tabs without renderer I/O.

LSP didChange notifications now carry the smallest changed range derivable from the old/current
snapshots. Workspace-local and static-extension language configuration drives bracket/surrounding
pairs, line/block comments, word selection, indentation regexes, and safe Enter append/remove
actions when available. Workspace/static-directory or VSIX snippets expand on Tab as one undoable
transaction, and matching static themes map to semantic terminal roles. Basic VS Code `when`
predicates and two-stroke key chords are evaluated by root dispatch.

File and directory resource WorkspaceEdit operations (create/rename/delete) are now applied with
trusted-root validation and rollback on operation failure. Rename and code-action edits targeting
unopened documents use the same background worker
as server-originated edits.
Revoking Workspace Trust shuts down persistent LSP sessions and rejects late server edits.
