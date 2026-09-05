# Application events

Role: typed results returned from services and policy enforcement. Save success/failure events carry
the target path and safe structured output rather than source text or credentials. Events return to
the sole state-mutation path. Git status and Explorer refresh completions carry typed payloads,
including the data needed to populate the pure Git dashboard model.
Clipboard completion events never include clipboard contents in failure messages; successful reads
carry text only to the pending paste transaction, and successful writes acknowledge cut semantics.
Language diagnostic notifications are converted from negotiated LSP positions into editor ranges
before being stored in root state; malformed ranges are ignored safely.
`DocumentSavedAs` changes the active tab path only after the workspace adapter confirms success.
`FileOperationCompleted` updates the active path when a file is renamed or moved and refreshes the
Explorer; `FileOperationFailed` preserves the result as a typed Output message.
`FileOperationCompleted` updates the active path when a file is renamed or moved and refreshes the
Explorer; `FileOperationFailed` preserves the result as a typed Output message.
LSP responses retain method and document version so stale results are ignored; initialization
records the negotiated position encoding.
Syntax updates are accepted only for the active document/version, while search results are filtered
by session id before entering the pure workspace UI model.
LSP responses retain method and document version so stale results are ignored before updating root
language state; initialization also records the negotiated position encoding.
Persistent language-server requests are represented by `LspServerRequest` and are answered only
after root policy applies a safe workspace edit.
`LspWorkspaceEditApplied` reports completion for client-originated edits that were dispatched to
the background worker, including typed failure reasons.
