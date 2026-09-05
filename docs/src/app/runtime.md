# Application runtime

Role: drives action/state/effect/event/render flow through injected ports. It renders once on startup,
dispatches effects outside state transitions, and restores the terminal after both success and normal
runtime errors. Error variants preserve both runtime and restoration failures. The root frame builder
composes the pure shell/editor views from immutable state and terminal capabilities, including
persistent Explorer/Output visibility, tabs, and horizontal/vertical split panes alongside the
command-palette model. Syntax highlights, folds, bracket pairs, and active workspace search ranges
are projected from immutable service snapshots, including negotiated semantic-token spans. Status
rendering projects the active tab's detected encoding and line-ending metadata. `run_interactive` uses one adapter for rendering and normalized input and
blocks while idle instead of polling. Dispatchers may poll typed completion events from background
work. Diagnostics are projected into the editor markers, status counts, and Problems bottom panel.
The recovery-enabled interactive variant persists an atomic session checkpoint after each
action so unexpected termination can restore unsaved buffers. Search, Git, and interactive
language-result panels are blitted from pure renderers after shell layout computation. Headless tests cover enter, frame
production, editing, quit, cleanup, and split/session round trips.
Deferred effects emitted while applying asynchronous events are dispatched by the runtime, allowing
format-on-save/paste and other service chains to remain outside the UI update path.
The same deferred-effect dispatch applies to responses for persistent language-server requests.
Filesystem operation prompts are projected into the Output panel with explicit Command Palette
confirm/cancel actions; only a confirmed plan reaches the background dispatcher.
