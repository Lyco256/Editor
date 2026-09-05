# Final integration results

Wave 2 integration is verified on `devenv` (commit `1887f97`). It has not yet been promoted to
`main`, because release-blocking evidence and promotion remain. On this
verified integration state, format check, workspace Clippy with
warnings denied, all workspace tests, source/document mirror check, release build, and redirected
headless startup all pass through `tools/verify.ps1`. Root integration now covers persistent editing,
atomic save effects, dirty-quit protection, Save As, split/session recovery, clipboard transactions,
manual formatter transactions, project replacement effects, command palette routing, native watcher
abstraction, and session recovery.

The release binary was launched without arguments in the current environment and exited 0 through the
safe headless startup path. A PTY-backed smoke run, idle memory/CPU samples, and successful Inno
Setup compilation are recorded in the smoke/performance documents; native 10 MiB typing-latency
profiling remains outstanding.
