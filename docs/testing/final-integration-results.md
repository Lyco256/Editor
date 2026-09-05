# Final integration results

Wave 2 is merged into `devenv` and promoted identically to `main` at the final verified commit. On this
verified integration state, format check, workspace Clippy with
warnings denied, all workspace tests, source/document mirror check, release build, and redirected
headless startup all pass through `tools/verify.ps1`. Root integration now covers persistent editing,
atomic save effects, dirty-quit protection, command palette routing, native watcher abstraction,
and session recovery.

The release binary was launched without arguments in the current environment and exited 0 through the
safe headless startup path. Interactive Windows Terminal smoke and resident-memory/idle-CPU/10 MiB
latency measurements remain unavailable in this headless execution environment and are recorded as
such in the smoke/performance documents.
