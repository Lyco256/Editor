# Application state

Role: authoritative mutable application state and pure transitions. Workers never receive mutable
access. The trust invariant blocks external effects while untrusted and records a visible warning.
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
Workspace roots and Explorer entries are retained in root state; `OpenPath` and `AddWorkspaceRoot`
actions refresh the projection without letting views perform filesystem I/O.
`apply_editor_action`, `apply_language_action`, and `apply_git_action` are typed adapters for the
app-ui models; Git and language-process requests are converted to trust-gated structured effects.
`open_tab`, `SwitchTab`, and the session serializer retain each open buffer and active-tab index,
so restart restores tab order and unsaved contents rather than only the foreground file.
Unit tests cover editing, dirty protection, blocked and authorized paths, plus clean input shutdown.
