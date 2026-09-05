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

The LSP client exposes server-originated requests, but root bootstrap does not yet handle
`workspace/applyEdit`; only active-document edits from client-originated rename/code-action results
are applied (cross-document edits remain preview-only). Server-requested workspace edits remain a
release-blocking implementation gap in addition to the native profiling and promotion gates.

The standalone LSP client also has protocol methods that are not yet reachable from root actions
(completion resolve, prepare rename, declaration/implementation navigation, and range formatting),
and syntax structural-selection ranges are not wired to an editor action. These remain functional
integration gaps.
