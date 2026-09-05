# Application runtime

Role: drives action/state/effect/event/render flow through injected ports. It renders once on startup,
dispatches effects outside state transitions, and restores the terminal after both success and normal
runtime errors. Error variants preserve both runtime and restoration failures. The root frame builder
composes the pure shell/editor views from immutable state and terminal capabilities, including
persistent Explorer/Output visibility, tabs, and horizontal/vertical split panes alongside the
command-palette model. `run_interactive` uses one adapter for rendering and normalized input and
blocks while idle instead of polling. Dispatchers may poll typed completion events from background
work. Diagnostics are projected into the editor markers, status counts, and Problems bottom panel.
The recovery-enabled interactive variant persists an atomic session checkpoint after each
action so unexpected termination can restore unsaved buffers. Headless tests cover enter, frame
production, editing, quit, cleanup, and split/session round trips.
