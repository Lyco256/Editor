# `editor` module

Role: pure editor viewport rendering and editor-local action types.

This module owns the text-viewport state used by the UI shell, including line numbers, gutter
markers, selections, multiple cursors, fold presentation, syntax/semantic spans, bracket/search
highlighting, overview ruler markers, and status-bar summary data.

Important types:

- `TextViewport` for scroll position.
- `MarkerKind`, `MarkerSpan`, and `SemanticMarkerSet` for typed language/Git/search marker inputs.
- `DiagnosticCounts` and `EditorStatusData` for the status summary.
- `EditorViewportState` for deterministic framebuffer rendering.
- `EditorAction` for normalized viewport intents.

Invariants:

- The module never performs filesystem, Git, LSP, or terminal I/O.
- Rendering is derived entirely from state passed in by the caller.
- Folding hides collapsed ranges and keeps the first line visible as the anchor line.
- Wide and combining Unicode sequences are rendered deterministically from the text snapshot.

Data flow:

- `TextSnapshot` provides read-only text access and display-column calculations.
- Selections, folds, and semantic markers are projected onto the visible viewport.
- Syntax/semantic foreground roles are projected from immutable logical ranges and are layered
  below selections, search matches, and bracket emphasis.
- The renderer writes semantic `Cell` values into the framebuffer for the shell to compose.

Concurrency:

- Pure view logic only. The module owns no background tasks or mutable shared services.

Error behavior:

- Snapshot rendering uses safe clipping and skips off-screen content.
- Invalid sample state in tests is surfaced through typed text-buffer errors, not panics in the
  production renderer.

Dependencies:

- `editor-core` for snapshots, selections, folds, and text ranges.
- `editor-types` for semantic roles, terminal capabilities, and typed editor actions.
- `terminal-backend` for framebuffer cells.

Tests:

- status-bar summary formatting
- line parsing and trailing empty-line handling
- 80x24 framebuffer snapshot with folds, Unicode, selections, and overview markers
- color-depth semantic summary output
