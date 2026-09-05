# Final integration results

Wave 2 integration is verified on the current tip of `devenv`. It has not yet been promoted to
`main`, because release-blocking evidence and promotion remain. On this
verified integration state, format check, workspace Clippy with
warnings denied, all workspace tests, source/document mirror check, release build, and redirected
headless startup all pass through `tools/verify.ps1`. Root integration now covers persistent editing,
atomic save effects, dirty-quit protection, Save As, split/session recovery, clipboard transactions,
manual formatter transactions, project replacement effects, command palette routing, native watcher
abstraction, and session recovery.

The release binary was launched without arguments in the current environment and exited 0 through the
safe headless startup path. A PTY-backed smoke run, idle memory/CPU samples, a five-iteration
Windows Terminal 10 MiB process-level open/force-close sample, and successful Inno Setup compilation
are recorded in the smoke/performance documents; native 10 MiB typing-latency and clean interactive
open/close profiling remain outstanding.

Server-originated `workspace/applyEdit` requests now pass through root validation, a background
atomic-save worker, open-tab refresh, and a typed JSON-RPC response. Multi-document edits and
dirty-buffer rejection are covered by deterministic tests. Root commands also expose completion
resolve, prepare rename, declaration/implementation navigation, range formatting, and
syntax-aware structural selection expansion.

Trusted language-server requests now reuse the persistent client session when available. Root
queues workspace-folder, didOpen, didChange, didSave, and didClose notifications; deterministic
state coverage verifies the lifecycle notifications are emitted. Directory resource operations
remain explicitly rejected and are still outside the complete WorkspaceEdit surface.

File resource WorkspaceEdit operations (create/rename/delete) are now applied with trusted-root
validation and rollback on operation failure. Directory resource operations remain explicitly
rejected. Rename and code-action edits targeting unopened documents use the same background worker
as server-originated edits.
Revoking Workspace Trust shuts down persistent LSP sessions and rejects late server edits.
