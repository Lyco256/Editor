# Differential rendering

Role: turns framebuffer changes into the smallest practical ANSI/VT payload for the active terminal.

Important types:

- `DifferentialRenderer` stores the previous frame snapshot and the terminal writer.
- `Theme`, `ResolvedColor`, and `ResolvedUnderline` drive semantic-to-terminal style conversion.
- `set_theme` replaces semantic colors at runtime and invalidates the previous-frame snapshot.

Invariants:

- Identical frames produce no terminal output.
- A frame size change forces a full redraw.
- Wide-cell continuation cells are never emitted as independent characters.
- A frame update is only committed as the new snapshot after the entire write succeeds.

Data flow:

- The renderer compares the next framebuffer with the previous one.
- Changed cells are emitted with explicit cursor positioning and reset state.
- Semantic colors resolve through the current terminal capability set before writing escape sequences.

Error behavior:

- Writes return the underlying `io::Error`.
- The previous snapshot is left untouched when a write fails so the next render can retry from a known
  state.

Dependencies:

- `editor-types` for semantic roles and terminal capability metadata.
- The local capability and framebuffer modules for style resolution and frame access.

Tests:

- Identical frames emit no payload.
- Small edits emit bounded changes rather than a full redraw.
- Diagnostic cells use the curly underline path when the capability advertises it.
