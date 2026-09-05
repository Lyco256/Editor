# Application state

Role: authoritative mutable application state and pure transitions. Workers never receive mutable
access. The trust invariant blocks external effects while untrusted and records a visible warning.
Trust is keyed by canonical workspace roots through the persisted `TrustStore`; changing trust
updates both the active policy bit and the store entry.
The active `TextBuffer` is retained in root state so character input, smart pairs, navigation,
undo/redo, and dirty tracking survive frame renders. Control-Q and Control-C exit only when the
buffer is clean; dirty quits surface a warning. `session_state` and `restore_session` bridge the
active editor (including selections and unsaved text) to the versioned, atomic recovery format.
Ctrl+B/Ctrl+J toggle the Explorer and Output panel, while Ctrl+P opens a keyboard-driven command
palette whose commands route back through the root transition path.
Git Changes, Diff, Branches, Stashes, History, Commit, and Conflicts each have command-palette
routes that select the corresponding dashboard view without giving the UI direct process access.
Language Problems, hover, signature help, completion, code action, restart, and dismiss actions
are likewise exposed through typed command-palette routes.
Request commands for completion, hover, signature, definition, references, rename preview, code
actions, inlay hints, symbols, and formatting construct `LanguageEffectRequest` values with the
current document version and logical cursor position.
Trusting a file workspace discovers a conventional language server on `PATH`; its startup and crash
events update the visible `LanguageServerStatus` without allowing untrusted process execution.
Left-click and drag input is translated from screen cells to logical buffer selections with bounded
layout offsets, keeping mouse selection in the same state machine as keyboard editing.
Mouse clicks on the tab strip switch tabs, and drag gestures adjust an active split ratio with
bounded 10–90% limits.
Workspace roots and Explorer entries are retained in root state; `OpenPath` and `AddWorkspaceRoot`
actions refresh the projection without letting views perform filesystem I/O.
`apply_editor_action`, `apply_language_action`, and `apply_git_action` are typed adapters for the
app-ui models; Git and language-process requests are converted to trust-gated structured effects.
Git hunk staging/unstaging and confirmed discard retain their typed patch/plan payloads through
dedicated effects rather than lossy shell command strings. LSP responses are retained by method and
also projected into versioned app-ui completion, hover, navigation, rename, code-action, hint,
symbol, and formatting models.
Git diff/conflict navigation resolves repository-relative paths through the root tab-opening path,
so the dashboard never performs filesystem access itself. Large-file mode remains editable while
syntax refresh effects carry an explicit suppression flag and no semantic LSP request is emitted.
Semantic-token delta streams are decoded with the negotiated UTF-8/UTF-16 position encoding,
versioned, and retained as UI spans; malformed tuples are rejected without changing the document.
The last interactive language result selects a rendered language panel through the root state.
`language_model` exposes that immutable versioned view to host integrations without granting any
worker or view direct process access.
`syntax_snapshot` exposes the latest parser snapshot for deterministic host-level validation.
Language panel actions (completion, hover, signature help, navigation, rename, code actions,
inlay hints, symbols, formatting, restart, and dismissal) are routed through root state; request
commands include document and workspace symbols.
Opening or switching to a document loads applicable `.editorconfig` sections; indentation, charset
fallback, end-of-line policy, trailing-whitespace trimming, and final-newline policy are applied to
the active document while invalid properties surface typed warnings. Save normalization is recorded
as one undoable buffer transaction before bytes are written.
`open_tab`, `SwitchTab`, and the session serializer retain each open buffer, detected encoding/BOM/
line-ending metadata, and active-tab index,
including an optional horizontal/vertical split, split ratio, and secondary tab. Session restore
honors persisted tab order, all selections, and recovered text even when the original file is
temporarily missing. Split/close/resize actions remain pure state transitions.
Clipboard copy/cut/paste use typed effects; cut deletes only after a successful write and paste
applies returned text as one undoable transaction. `editor.format` uses the authorized external
formatter fallback when configured and rejects stale results before applying a transaction.
Unit tests cover editing, dirty protection, blocked and authorized paths, plus clean input shutdown.
Workspace Quick Open and streaming project search are routed through typed root actions/effects,
and syntax refreshes are scheduled after document-version changes.
Language effect requests are converted to structured LSP methods and trust-gated, with negotiated
position encoding applied to diagnostic range conversion.
Git status events populate the pure dashboard model and successful Git mutations schedule a status
refresh.
