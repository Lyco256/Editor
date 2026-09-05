# Framebuffer storage

Role: stores the virtual terminal image as fixed-size rows of semantic cells.

Important types:

- `Cell` holds the grapheme text, semantic foreground and background roles, bold state, and wide-cell
  continuation flag.
- `Framebuffer` owns the row-major cell grid.
- `FramebufferError` reports bounds and grapheme validation failures.

Invariants:

- One framebuffer cell owns exactly one grapheme cluster.
- Zero-width graphemes are rejected.
- Wide graphemes mark their trailing cells as continuations.
- Replacing a grapheme repairs any overlapping wide-cell ownership so the frame stays coherent.

Data flow:

- Higher layers build the next screen image by writing cells into the framebuffer.
- The renderer reads the framebuffer and converts the semantic cells into terminal output.
- Clearing a coordinate removes the full grapheme that owns that coordinate, not just the display cell
  at the coordinate itself.

Error behavior:

- Out-of-bounds access is a typed error.
- Invalid graphemes leave the framebuffer unchanged.

Dependencies:

- `editor-types` for semantic style roles.
- `unicode-segmentation` and `unicode-width` for grapheme and display-width handling.

Tests:

- Cell replacement works at arbitrary coordinates.
- Wide graphemes mark and repair continuation cells.
- Combining sequences behave like a single safe display cell.
- Invalid grapheme input does not mutate the existing frame.
