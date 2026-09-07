# 035 — caret selection and Unicode cell rendering

Owner: POINTER worker

Writable paths:
- `crates/app-ui/src/editor/cursor.rs`
- `crates/app-ui/src/editor/selection.rs`
- `crates/app-ui/src/editor/text_metrics.rs`
- matching docs/tests

## Objective

Preserve source glyphs under carets/selections and unify grapheme/display-cell metrics.

## Required implementation

1. Primary caret renderer returns terminal cursor screen position only; it never writes `▌` or another replacement glyph.
2. Selection renderer preserves original grapheme and applies SelectionForeground/SelectionBackground.
3. Secondary cursors preserve original grapheme and apply dedicated secondary-cursor style; at EOL style one blank cell if available.
4. Use Unicode grapheme segmentation and terminal display width consistently.
5. Never position primary/secondary caret on a wide glyph continuation cell.
6. Combining grapheme occupies the display width of its composed grapheme.
7. Text truncation does not split grapheme.
8. Remove legacy `render_cursors` glyph-overwrite implementation once replacement is wired.

## Required tests

- `M011`–`M015`
- `Q001_UNICODE_CHROME_WIDTH` relevant editor portion
- ASCII/CJK/emoji/combining source fixtures

## Done only when

source characters remain readable under all cursor/selection states and cell geometry is Unicode-safe.
