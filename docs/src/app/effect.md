# Application effects

Role: typed descriptions of asynchronous work. Process specifications keep executable and arguments
separate, while `SaveDocument` carries an immutable document snapshot to the workspace adapter. The
`requires_trusted_workspace` invariant makes every external process kind subject to the root trust
policy before dispatch. `RefreshGitStatus` is also trust-gated and returns its result through a
typed event. `RefreshExplorer` carries workspace roots to a background filesystem traversal.
`ClipboardWrite` and `ClipboardRead` carry clipboard operations through the terminal-backend
adapter; cut is acknowledged before its deletion transaction is committed.
