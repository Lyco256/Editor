# Capability detection and style fallback

Role: converts environment hints and semantic styles into terminal-friendly capabilities and color
choices.
The built-in dark theme uses a true black editor background and distinct semantic chrome roles.

Important types:

- `RgbColor` stores RGB values independent of palette depth.
- `ResolvedColor` records whether a color stays true color or is quantized.
- `ResolvedUnderline` records whether the underline path is curly, straight, or emphasis fallback.
- `Theme` maps `StyleRole` values to semantic colors.

Invariants:

- Capability detection stays conservative. Unknown terminals receive portable ANSI defaults.
- Color fallback is deterministic so the same RGB input always resolves to the same ANSI index.
- Underline fallback never drops all diagnostic emphasis.

Data flow:

- Environment markers such as `TERM`, `COLORTERM`, `WT_SESSION`, `KITTY_WINDOW_ID`, and
  `WEZTERM_PANE` feed `detect_capabilities`.
- The renderer asks `resolve_color` and `resolve_underline` before writing escape sequences.
- `Theme` supplies the semantic colors that the renderer resolves for each cell.

Error behavior:

- This module does not return runtime errors. It only performs deterministic classification and
  quantization.

Dependencies:

- `editor-types` for `ColorDepth`, `StyleRole`, `TerminalCapabilities`, `TerminalFeature`, and
  `UnderlineStyle`.

Tests:

- Quantization is deterministic and stable for known palette colors.
- Underline fallback keeps diagnostic emphasis when curly underlines are unavailable.
