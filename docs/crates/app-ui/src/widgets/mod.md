# `widgets` module

Role: shared terminal UI helpers for `app-ui`.

This module owns the reusable rectangle geometry, framebuffer drawing helpers, command-palette
filtering state, generic picker/command-registry contracts, and deterministic framebuffer snapshot formatting used by the shell and editor
tests.
The palette can materialize the same typed registry view used by keybinding dispatch.

Important types:

- `Rect` for layout geometry and split calculations.
- `CommandEntry`, `CommandMatch`, and `CommandPaletteState` for palette filtering and selection.
- Helper constructors for styled `Cell` values and rectangle fills/borders.

Invariants:

- Helpers never touch the filesystem, terminal, or process APIs.
- Palette filtering is deterministic and keeps disabled commands visible.
- Snapshot formatting is semantic: it records roles and visible cells instead of escape bytes.

Data flow:

- Shell and editor code create `Cell` values and write them into the terminal framebuffer through
  the helpers here.
- Tests render a `Framebuffer`, then use `frame_snapshot` or `semantic_palette_snapshot` for
  deterministic golden output.

Concurrency:

- Pure value helpers only. No shared mutable state and no background work.

Error behavior:

- The drawing helpers ignore out-of-bounds writes that the caller has already clipped away.
- Palette filtering returns empty matches rather than executing commands.

Dependencies:

- `editor-types` for command identifiers and semantic roles.
- `terminal-backend` for `Cell`, `Framebuffer`, theme color resolution, and semantic palette data.

Tests:

- palette filtering and activation selection
- framebuffer semantic snapshot formatting
- theme depth mapping for true-color and reduced-color output
# Generic picker and command registry

`GenericPicker` provides typed stable rows, query filtering, bounded selection, accept and cancel;
`CommandRegistry` is the shared registration surface for palette and keybinding routes.
# Display-cell-safe chrome writing

`write_text` segments grapheme clusters, uses terminal display widths, clips to framebuffer bounds,
and never splits a wide user string across a continuation cell.
