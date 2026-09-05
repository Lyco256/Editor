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
`open_tab`, `SwitchTab`, and the session serializer retain each open buffer and active-tab index,
including an optional horizontal/vertical split, split ratio, and secondary tab. Session restore
honors persisted tab order, all selections, and recovered text even when the original file is
temporarily missing. Split/close/resize actions remain pure state transitions.
Clipboard copy/cut/paste use typed effects; cut deletes only after a successful write and paste
applies returned text as one undoable transaction. `editor.format` uses the authorized external
formatter fallback when configured and rejects stale results before applying a transaction.
Unit tests cover editing, dirty protection, blocked and authorized paths, plus clean input shutdown.
