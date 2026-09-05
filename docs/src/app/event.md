# Application events

Role: typed results returned from services and policy enforcement. Save success/failure events carry
the target path and safe structured output rather than source text or credentials. Events return to
the sole state-mutation path. Git status and Explorer refresh completions carry typed payloads.
Clipboard completion events never include clipboard contents in failure messages; successful reads
carry text only to the pending paste transaction, and successful writes acknowledge cut semantics.
Language diagnostic notifications are converted from negotiated LSP positions into editor ranges
before being stored in root state; malformed ranges are ignored safely.
`DocumentSavedAs` changes the active tab path only after the workspace adapter confirms success.
