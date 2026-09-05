# Process bootstrap

Role: owns process-level startup, positional path parsing, terminal-size fallback, and exit-code
conversion. A path (or an interactive no-argument terminal) launches the crossterm adapter for a file
or workspace; a redirected launch uses the deterministic headless lifecycle. Save effects run on a
Tokio background worker and return typed completion events. Interactive no-argument startup restores
the latest application-data session (falling back to the current directory), and shutdown persists
the resulting session atomically. Invalid options return a usage error without starting a
subprocess. A panic hook performs best-effort terminal restoration and safe crash logging. The
bootstrap loads and saves the canonical-path workspace trust store beside recovery data.
Explorer refreshes are dispatched to Tokio background workers and returned as typed events;
filesystem traversal is not performed by the renderer. Clipboard reads and writes are performed by
the system adapter on a worker and return typed success/failure events; clipboard contents are not
logged. Trusted language-server effects now use `lsp-client` stdio framing and initialization,
forwarding stderr/crash events and requesting shutdown when the dispatcher is dropped.
Server-originated requests are forwarded to root as typed events; validated `workspace/applyEdit`
effects run on a background worker and return a JSON-RPC response through the owning client session.
Text edits and file/directory resource operations use canonical trusted-root checks; failed
resource operations roll back their journaled changes, and rollback failures are returned as part
of the typed error instead of being silently ignored.
Trust revocation drains and cleanly shuts down all persistent language-server sessions.
Requests and document lifecycle notifications reuse the persistent client session when available;
the one-shot request path remains only as a startup fallback.
Syntax refreshes run through Tokio background workers, while project search streams typed
matches and supports cancellation through a cloneable service handle. Structured LSP request effects
use a bounded current-thread Tokio runtime and never concatenate shell commands. Git hunk and
confirmation-bound discard effects invoke the typed `vcs-git` APIs with patch data on stdin and
return `EffectCompleted` or structured failure output.
After a path is selected it loads the workspace `.vscode/settings.json` as a JSONC data layer;
resolved format, indentation, line-number, and large-file settings are applied to root state, while
malformed settings remain visible as warning output. A workspace-local
`.vscode/language-configuration.json`, when present, is parsed through the static compatibility
boundary and its bracket pairs are applied to smart editing; parse failures and unsupported fields
are surfaced as typed output instead of enabling unsafe defaults silently. If the workspace file is
absent, static language configurations from `.vscode/extensions` are considered (both unpacked
directories and path-safe `.vsix` archives extracted into the OS cache); malformed extension
metadata is reported as a compatibility warning rather than being silently discarded.
Static language contributions are matched by declared language id, filename, or extension, so
custom file types can receive their configuration and snippets even when Tree-sitter has no
built-in parser.
Workspace and static-extension snippet files are loaded without executing extension code; matching
prefixes expand through a single undoable root transaction. Valid static theme JSON from workspace
files or unpacked/archived extensions is mapped to semantic terminal roles before the interactive
renderer starts, while malformed themes become compatibility warnings.
