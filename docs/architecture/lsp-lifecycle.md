# LSP lifecycle

The LSP adapter owns a child process and JSON-RPC framing on a Tokio task. The root application now
trust-gates server startup, performs initialize/initialized, forwards protocol events, projects live
diagnostics, and exposes crash/exit status without terminating the editor. Results are accepted only
when their request and document version are still current. Completion, hover, navigation, rename,
code-action, semantic-token, and inlay-hint UI/request wiring remain release-blocking follow-up
work; the adapter and protocol types are present so those additions stay behind the same boundary.
