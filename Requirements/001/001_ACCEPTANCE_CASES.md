# 001 — frozen acceptance cases

The setup run implements automated tests for every ID below before production fixes begin.

Each case ID is also placed verbatim in `tools/requirements-001/required-cases.txt`.

The final verifier must prove that every ID exists and passes.

## Resize and geometry

- `R001_RESIZE_IMMEDIATE` — a `Resize(80,24)` received after rendering 120x40 causes the next presented frame to be exactly 80x24 without waiting for another input.
- `R002_RESIZE_SEQUENCE` — 120x40 -> 55x12 -> 160x50 -> 80x24 produces exact frame sizes in sequence.
- `R003_STATUS_BOTTOM_ROW` — when status is visible, its rectangle ends at `rows - 1` after every resize.
- `R004_RENDER_HIT_SAME_LAYOUT` — clicking the center of every visible major region resolves to the same region identity that rendered there.
- `R005_NO_STALE_GEOMETRY` — a coordinate that belonged to Explorer before resize is not interpreted using the old Explorer rectangle after layout changes.
- `R006_EXACT_TAB_HIT` — variable-width tab rectangles activate the tab whose rendered rectangle contains the click.
- `R007_SPLIT_EXACT_HIT` — pointer target uses actual split geometry, not half-screen constants.
- `R008_NO_FILLER_REGION` — normal workbench cells are owned by useful content/chrome/separators; no unexplained repeated-rule region exists.

## Keyboard navigation

- `K001_LEFT_RIGHT_GRAPHEME` — Left/Right move by full grapheme.
- `K002_COLLAPSE_SELECTION` — Left collapses non-empty selection to start; Right to end without Shift.
- `K003_SHIFT_EXTENDS` — Shift+arrow extends from anchor.
- `K004_PREFERRED_COLUMN_EMPTY` — `abcde\n\nabcde`, start line 1 display col 5: Down -> blank line col 0 with goal 5; Down -> line 3 col 5.
- `K005_PREFERRED_COLUMN_SHORT` — long/short/long preserves the original goal column.
- `K006_PREFERRED_COLUMN_TABS` — goal is display-cell based using effective tab width.
- `K007_PREFERRED_COLUMN_MULTI` — each cursor keeps its own vertical goal.
- `K008_HOME_SMART` — Home moves to first non-whitespace; repeated Home toggles to absolute line start.
- `K009_END_LINE` — End moves to logical/visual line end as specified by the non-wrapped initial implementation.
- `K010_CTRL_HOME_END` — Ctrl+Home/End move to document start/end.
- `K011_PAGE_UP_DOWN` — PageUp/PageDown move by the supplied visible-page height and keep preferred column.
- `K012_CTRL_UP_DOWN_SCROLL` — Ctrl+Up/Down scroll viewport without moving text selection.
- `K013_WORD_LEFT_RIGHT` — Ctrl+Left/Right use word boundaries.
- `K014_WORD_DELETE` — Ctrl+Backspace/Delete delete to word boundary.
- `K015_DELETE_GRAPHEME` — plain Delete removes one complete grapheme, not one scalar.
- `K016_BACKSPACE_GRAPHEME` — Backspace removes one complete grapheme.
- `K017_TAB_INDENT` — Tab indents selection or inserts to next indent stop from effective settings.
- `K018_SHIFT_TAB_OUTDENT` — Shift+Tab outdents without deleting non-indent text.
- `K019_MOVE_LINES` — Alt+Up/Down moves selected/current line block.
- `K020_COPY_LINES` — Shift+Alt+Up/Down copies line block.
- `K021_INSERT_LINE` — Ctrl+Enter / Ctrl+Shift+Enter insert below/above.
- `K022_SELECT_LINE_ALL` — Ctrl+L selects line; Ctrl+A selects document.
- `K023_NO_SILENT_NAV_NOOP` — Home/End/PageUp/PageDown and all required navigation keys either perform their specified action or are explicitly unavailable before dispatch; they never fall through as successful no-ops.

## Pointer and caret

- `M001_CLICK_TEXT` — click a rendered grapheme -> valid caret boundary.
- `M002_CLICK_RIGHT_TO_EOL` — click after line content -> EOL.
- `M003_CLICK_EMPTY_LINE` — click blank logical line -> that line's EOL.
- `M004_CLICK_BELOW_TO_EOF` — click inside editor below final document line -> EOF.
- `M005_DRAG_CLAMP_EOL_EOF` — drag beyond content clamps safely.
- `M006_SHIFT_CLICK` — Shift+click extends primary selection from anchor.
- `M007_ALT_CLICK_CURSOR` — Alt+click adds secondary cursor.
- `M008_DOUBLE_CLICK_WORD` — double click selects the word at pointer.
- `M009_TRIPLE_CLICK_LINE` — triple click selects whole logical line.
- `M010_WHEEL_TARGET` — wheel over pane scrolls that pane; Explorer wheel scrolls Explorer.
- `M011_PRIMARY_CARET_PRESERVES_GLYPH` — source framebuffer glyph under primary caret is unchanged.
- `M012_PRIMARY_CARET_STEADY_BAR` — focused text editor requests a visible SteadyBar terminal cursor at the actual caret cell.
- `M013_SELECTION_PRESERVES_GLYPH` — selected cells retain original graphemes.
- `M014_SECONDARY_CURSOR_PRESERVES_GLYPH` — secondary cursor styling never replaces source glyph.
- `M015_CJK_COMBINING_POINTER` — click/caret/selection never target wide-character continuation or split combining grapheme.

## Workbench

- `U001_TOP_MENU` — normal layout visibly renders `File Edit Selection View Go Help`.
- `U002_MENU_KEYBOARD` — F10 and arrow/Enter/Escape menu navigation work.
- `U003_MENU_MOUSE` — click category/item and outside dismissal use exact rectangles.
- `U004_FILE_FOLDER_DISTINCT` — file vs directory remains clear with color removed; directories have `/` and expansion marker.
- `U005_NO_ICON_CHROME` — production UI chrome uses printable ASCII or U+2500–U+257F Box Drawing only.
- `U006_PREVIEW_REPLACE` — Explorer single-click opens clean Preview; next preview replaces it.
- `U007_PREVIEW_PROMOTE_DIRTY` — editing Preview promotes it before another preview can replace it.
- `U008_OPEN_PERMANENT` — double click/Enter produces permanent Open editor.
- `U009_PIN_STATE` — Pin/Unpin changes persistent tab disposition and visual label.
- `U010_GROUP_LOCAL_TABS` — each split editor group owns its tab strip.
- `U011_NO_PHANTOM_PANE` — one editor group fills available editor area; no empty second pane.
- `U012_DENSE_LAYOUT` — normal pane uses only required one-cell separators/overview and no outer pane box.
- `U013_BLACK_EDITOR_BACKGROUND` — built-in dark EditorBackground is exactly #000000.
- `U014_CONTRAST` — required foreground/background pairs meet defined True Color contrast and remain distinct in 256/16-color fallback.
- `U015_STATUS_LIVE` — status cursor line/column/selection reflect focused document and move after input/resize.

## Projection correctness

- `P001_ACTIVE_BUFFER_AUTHORITATIVE` — rendering uses persistent tab buffer snapshot/selections; changing only stale compatibility `active_text` cannot replace the rendered authoritative document.
- `P002_PANE_VIEW_STATE_PERSISTENT` — scroll/folds survive unrelated frames and are pane-specific.
- `P003_SECONDARY_PANE_ISOLATION` — two different documents in splits render their own syntax/search/diagnostics/inlay/status data.
- `P004_SEARCH_EXACT_RANGE` — two identical matches on one line remain two exact distinct markers.
- `P005_SEARCH_STALE_DIRTY` — stale on-disk search hit is not relocated to first equal substring in dirty buffer.
- `P006_GIT_HUNK_LINES` — one-line tracked edit in 100-line file marks actual hunk lines, not whole file.
- `P007_GIT_ADD_DELETE` — additions and deletions map to deterministic new-file/nearest-survivor gutter anchors.
- `P008_LSP_OVERLAY_CARET_ANCHOR` — completion/hover/signature/code-action overlay follows actual caret after Explorer toggle, scroll, split, and resize.
- `P009_LSP_OFFSCREEN_DISMISS` — ephemeral caret-context overlay is dismissed when its document anchor is offscreen/stale.
- `P010_LSP_DOCUMENT_IDENTITY` — document-scoped result keeps the requesting document ID/version.
- `P011_CONTEXTUAL_NOT_LANGUAGE_PANEL` — completion/hover/signature/Quick Fix/rename do not require a generic Language bottom panel.

## Robustness

- `Q001_UNICODE_CHROME_WIDTH` — CJK/data text and combining text in overlays are clipped by display cells, not Rust char count.
- `Q002_RESIZE_PROPERTY` — generated non-zero terminal sizes satisfy rectangle containment and hit/render identity invariants.
- `Q003_NO_FIXED_GEOMETRY_PATTERNS` — forbidden legacy fixed-coordinate source patterns are absent.
- `Q004_NO_CURSOR_GLYPH_PATTERN` — the old block/bar glyph cursor path is absent from production chrome.
- `Q005_NO_REQUIRED_IGNORES` — no 001 acceptance test is ignored/skipped/conditional.
- `Q006_FROZEN_ORACLE_UNCHANGED` — frozen acceptance artifacts have no diff from baseline.
