# Application runtime

Role: drives action/state/effect/event/render flow through injected ports. It renders once on startup,
dispatches effects outside state transitions, and restores the terminal after both success and normal
runtime errors. Error variants preserve both runtime and restoration failures. The root frame builder
composes the pure shell/editor views from immutable state and terminal capabilities, including
persistent Explorer/Output visibility, tabs, and horizontal/vertical split panes alongside the
command-palette model. Syntax highlights, folds, bracket pairs, and active workspace search ranges
are projected with exact byte ranges, and Git hunks become line-local gutter markers. Resize events
update dimensions immediately (zero-sized frames are ignored) and the terminal adapter presents a
native steady-bar cursor for the active editor.
are projected from immutable service snapshots, including negotiated semantic-token spans. Status
rendering projects the active tab's detected encoding and line-ending metadata. `run_interactive` uses one adapter for rendering and normalized input and
blocks while idle instead of polling. Dispatchers may poll typed completion events from background
work. Diagnostics are projected into the editor markers, status counts, and Problems bottom panel.
The recovery-enabled interactive variant persists an atomic session checkpoint after each
action so unexpected termination can restore unsaved buffers. Search, Git, and interactive
language-result panels are blitted from pure renderers after shell layout computation. Headless tests cover enter, frame
production, editing, quit, cleanup, and split/session round trips. Cursor-anchored contextual
language overlays are painted after shell and input surfaces.
Deferred effects emitted while applying actions or asynchronous events are dispatched by the runtime,
allowing format-on-save/paste, workspace search, and other service chains to remain outside the UI
update path.
The same deferred-effect dispatch applies to responses for persistent language-server requests.
Filesystem operation prompts are projected into the Output panel with explicit Command Palette
confirm/cancel actions; only a confirmed plan reaches the background dispatcher.
Quick Open and find/search/replace prompts are rendered as small overlays above the shell and are
driven by normalized keyboard/mouse input; Explorer focus/navigation is likewise represented in root
state, and its resulting open/expand actions dispatch deferred effects through this runtime.
# Rendering authority

Frame construction reads the persistent active tab buffer and pane viewport/folds directly; it
does not reconstruct a TextBuffer from the projected `active_text` field.
