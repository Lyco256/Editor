# `shell` module

Role: top-level terminal UI shell composition.

This module lays out the persistent Explorer sidebar, tab strip, split editor tree, bottom panel,
command palette, and status bar. It also normalizes keyboard and mouse interaction into shell
actions without performing external I/O.

`WorkbenchLayoutSnapshot` is the shared geometry contract for rendering and hit testing. It carries
the menu bar, exact tab rectangles, and all pane-local regions; `dispatch_mouse_at` consumes the
same terminal area rather than assuming a fixed size. The menu presents File, Edit, Selection,
View, Go, and Help.

Important types:

- `ShellFocus` for high-level focus routing.
- `SplitAxis` and `PaneNode` for recursive horizontal/vertical split rendering.
- `TabEntry`, `ExplorerState`, and `BottomPanelState` for shell chrome data.
- `ShellLayout` for collapse-aware geometry.
- `SplitHandleLayout` for exact one-cell draggable split separators.
- `ShellState` for the full shell view model.
- `ShellAction` for normalized user intents.

Invariants:

- The shell never talks to the filesystem, process APIs, or subsystems directly.
- Collapse order favors preserving an interactive editor viewport before shell chrome.
- Disabled or unavailable palette items remain visible but are rendered distinctly.
- Split rendering and palette filtering are deterministic from state alone.

Data flow:

- The shell computes a layout from the available framebuffer area.
- It renders explorer, tabs, editor panes, bottom panel, status bar, and palette into the frame.
- Mouse hits are translated into shell actions by pure hit-testing.
- Split separators are represented in the snapshot and hit-tested by their rendered rectangle;
  no fixed half-screen or tab-width arithmetic is used.

Concurrency:

- Pure view logic only. No background work and no shared mutable service access.

Error behavior:

- Geometry helpers clip conservatively and keep the editor visible when the viewport becomes tiny.
- Invalid user input is surfaced as actions or no-ops rather than terminal failures.

Dependencies:

- `editor-types` for key/mouse input and semantic roles.
- `editor` for viewport rendering and status data.
- `widgets` for geometry, palette filtering, and framebuffer drawing helpers.
- `terminal-backend` for the target framebuffer.

Tests:

- compact shell layout behavior
- keyboard and mouse action routing
- 120x40 framebuffer snapshot covering tabs, explorer, split panes, bottom panel, and status bar
# Pane routing

Editor leaves carry stable pane ids; recursive hit testing returns `FocusPane(id)` so nested split
geometry can route keyboard and mouse focus without relying on tab labels.
