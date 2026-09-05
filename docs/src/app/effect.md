# Application effects

Role: typed descriptions of asynchronous work. Process specifications keep executable and arguments
separate, while `SaveDocument` carries an immutable document snapshot plus encoding/BOM/EOL
metadata to the workspace adapter. The
`requires_trusted_workspace` invariant makes every external process kind subject to the root trust
policy before dispatch. `RefreshGitStatus` is also trust-gated and returns its result through a
typed event. `RefreshExplorer` carries workspace roots to a background filesystem traversal.
`ClipboardWrite` and `ClipboardRead` carry clipboard operations through the terminal-backend
adapter; cut is acknowledged before its deletion transaction is committed.
`SaveDocumentAs` uses the same atomic workspace writer and metadata while reporting a distinct completion event
so a failed Save As cannot retarget or dirty the active tab.
`FileOperation` carries a planned create/rename/move/delete operation and is emitted only after the
root state receives explicit confirmation; execution remains on the background dispatcher.
`FileOperation` carries a planned create/rename/move/delete operation and is emitted only after the
root state receives explicit confirmation; execution remains on the background dispatcher.
`RefreshSyntax` and `SearchWorkspace` keep parser/search work outside the state transition path;
search sessions can be cancelled with a typed effect.
`LspRequest` carries a structured JSON-RPC method/parameters payload and is trust-gated before any
server process is started.
`LspServerResponse` sends the result of a root-validated server-originated request back to the
persistent client.
`GitHunk` carries a parsed diff hunk to the Git adapter over stdin for stage/unstage, while
`GitDiscard` carries a confirmation-bound discard plan. Both are trust-gated and report completion
or typed failure events without constructing shell command strings.
