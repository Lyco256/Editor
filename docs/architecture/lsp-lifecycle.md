# LSP lifecycle

The LSP adapter owns a child process and JSON-RPC framing on a Tokio task. The root application
negotiates capabilities, sends lifecycle and document notifications, and applies only results whose
request and document version are still current. Startup is trust-gated; unavailable or crashed
servers become visible status/output events and can be restarted without terminating the editor.

