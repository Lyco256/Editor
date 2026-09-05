# LSP lifecycle

The LSP adapter owns a child process and JSON-RPC framing on a Tokio task. The root application
trust-gates server startup, performs initialize/initialized, forwards protocol events, projects live
diagnostics, and exposes crash/exit status without terminating the editor. Results are accepted only
when their request and document version are still current. Completion, hover, signature help,
navigation, references, rename, code actions, semantic tokens, inlay hints, document symbols, and
formatting are projected into versioned language views. Command-palette and typed `Action` routes
request these operations without allowing the UI to access the process directly.
