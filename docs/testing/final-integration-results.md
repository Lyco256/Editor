# Final integration results

Wave 2 integration is verified on `devenv` (commit `2e69cca`). It has not yet been promoted to
`main`, because release-blocking evidence and architecture work remain. On this
verified integration state, format check, workspace Clippy with
warnings denied, all workspace tests, source/document mirror check, release build, and redirected
headless startup all pass through `tools/verify.ps1`. Root integration now covers persistent editing,
atomic save effects, dirty-quit protection, Save As, split/session recovery, clipboard transactions,
manual formatter transactions, project replacement effects, command palette routing, native watcher
abstraction, and session recovery.

The release binary was launched without arguments in the current environment and exited 0 through the
safe headless startup path. Interactive Windows Terminal smoke and resident-memory/idle-CPU/10 MiB
latency measurements remain unavailable in this headless execution environment and are recorded as
such in the smoke/performance documents.
