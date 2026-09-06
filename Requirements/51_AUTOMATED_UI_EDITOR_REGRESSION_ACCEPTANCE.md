# Top Codex — automated UI/editor regression acceptance

## Purpose

This is the final automated acceptance gate for Requirements 42–50.

It is intentionally stronger than a screenshot-only check.

The implementation is accepted only when geometry, pointer hit testing, source-position mapping, navigation state, theme readability, and terminal presentation are all mechanically verified.

## 1. Repository quality gates

All pass:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
- source/document mirror verification
- repository-local verification scripts executable in the current environment

No required regression test is ignored/disabled.

## 2. Resize matrix

Test at least:

- 40x10
- 48x12
- 55x12
- 80x24
- 100x30
- 120x40
- 160x50
- 220x60

Also test a sequential resize chain containing both shrink and expand.

At each non-zero size verify:

- framebuffer exact dimensions,
- no out-of-bounds write,
- all non-overlay layout rectangles contained by frame,
- status at bottom when visible,
- menu at top when normal/compact policy requires it,
- active editor has non-negative usable geometry,
- click center of every visible region resolves to the same region identity,
- previous-size geometry is not accepted as current geometry.

## 3. Geometry property tests

Generate many valid terminal sizes within a bounded range.

For every generated size:

- layout is deterministic,
- non-overlay regions do not unexpectedly overlap,
- one editor remains usable when compact policy says it should,
- no phantom second pane exists,
- split separator thickness is exactly one cell,
- overview thickness is exactly one cell,
- group tabs belong to their group,
- no filler region is produced.

## 4. Hard-coded geometry regression guard

Add repository-local source checks or architecture tests that fail if production root/shell pointer routing reintroduces:

- `Rect::new(0, 0, 120, 40)` as a hit-test assumption,
- Explorer width constants as hit boundaries,
- half-screen split constants,
- fixed tab-index division,
- fixed global editor coordinate subtraction.

Numerical minimum layout constants inside the central layout engine are allowed.

## 5. Mouse regression matrix

Automated fake-pointer tests cover:

- exact source click,
- right whitespace -> EOL,
- empty line -> EOL,
- below last line -> EOF,
- drag after EOL,
- drag below EOF,
- Shift-click,
- Alt-click,
- double-click word,
- triple-click line,
- line-number click,
- fold click,
- overview click,
- editor wheel,
- Explorer wheel,
- bottom-panel wheel.

Repeat representative cases:

- before resize,
- after shrink,
- after expand,
- in vertical split,
- in horizontal split.

## 6. Preferred-column navigation matrix

Mandatory documents include:

- `abcde\n\nabcde`
- long/short/long lines
- leading tabs
- CJK full-width text
- combining grapheme text
- multiple cursors at different target columns.

Verify Up/Down/Page movement retains each preferred display column across clamps.

## 7. Conventional editor keyboard matrix

Automated tests cover all Requirement 47 commands and Shift-selection variants:

- arrows,
- Home/End,
- Ctrl+Home/End,
- word navigation,
- word deletion,
- PageUp/PageDown,
- Ctrl+Up/Down viewport intent,
- Tab/ShiftTab,
- move/copy line,
- insert line above/below,
- Select Line,
- Select All.

Mutating commands are tested with Undo/Redo.

## 8. Cursor/readability gate

For a frame containing ordinary ASCII, syntax-colored text, CJK, and combining sequences:

- primary cursor never changes the framebuffer source grapheme,
- non-empty selection never removes its source graphemes,
- secondary cursors never remove source graphemes,
- hardware cursor is `SteadyBar` while focused,
- hardware cursor is hidden during menu/modal focus.

## 9. Chrome glyph gate

Automated chrome-policy validation fails if new production UI decorations use forbidden icon-like/decorative Unicode.

Required structural UI uses ASCII and Box Drawing only.

Unicode remains unrestricted for data/user content.

## 10. Tab/Explorer gate

Verify:

- file and folder remain distinguishable with color disabled/reduced,
- directories have trailing `/`,
- `[+]`/`[-]` state matches expansion,
- Preview/Open/Pinned visual states differ,
- clean Preview replacement,
- dirty Preview promotion,
- double-click promotion,
- Pin/Unpin,
- pinned preserved by Close Others,
- exact tab hit rectangles under variable display widths,
- CJK filename tab geometry remains aligned.

## 11. Menu gate

Verify:

- normal menu labels exactly `File Edit Selection View Go Help`,
- F10 focus,
- access keys,
- left/right/up/down navigation,
- Enter execution,
- Escape close,
- disabled items,
- mouse selection,
- outside dismissal,
- compact `Menu` behavior.

Every executable menu row resolves to the authoritative command registry.

## 12. Theme gate

Verify:

- EditorBackground exactly black in default theme,
- required True Color text pairs >= 4.5 contrast,
- required ANSI256 text pairs >= 4.5 after safe pair resolution,
- critical ANSI16 foreground/background indices differ,
- selection text is readable,
- menu selected row is readable,
- Explorer directory/file/selected row is readable,
- active/inactive/preview/pinned tabs remain distinguishable.

## 13. Layout-density gate

At 80x24 with Explorer visible and bottom panel hidden:

- menu consumes one row,
- each editor group tab strip consumes one row,
- status consumes one row,
- no outer pane border consumes an extra row/column,
- no right-side filler strip exists beyond the one-cell overview,
- remaining available cells belong to useful Explorer/editor content or intentional separators.

## 14. Differential-render resize gate

Terminal-backend tests prove frame-size changes perform a safe full redraw and cannot index old framebuffer storage with new dimensions.


## 15. Git marker precision gate

Use deterministic `GitDiffFile` fixtures and/or temporary local Git repositories.

Verify:

- modifying one line in a 100-line tracked file does not mark all 100 lines,
- addition hunks mark only new-file added ranges,
- replacement hunks produce modified semantics for the affected new-file region,
- pure deletion hunks create a deleted marker at the nearest surviving new-file anchor,
- untracked-file behavior is deterministic,
- stage/unstage/discard refresh changes marker state.

A file-level `0..document_len` Git marker for an ordinary one-hunk modification fails acceptance.

## 16. Search marker precision gate

Use a line containing the same query at least twice.

Verify:

- backend search results preserve distinct exact byte ranges,
- UI/root projections preserve both identities,
- the two editor marker ranges are distinct,
- navigation chooses the selected result occurrence,
- a dirty buffer that invalidates an on-disk project-search hit does not repaint that hit at the first equal text occurrence.

A production use of `line_text.find(matched_text)` to locate a backend result fails the architectural regression check.

## 17. Contextual overlay coordinate gate

For completion/hover/signature/code-action overlay fixtures, verify screen placement after:

- Explorer visible vs hidden,
- viewport scrolled vertically,
- viewport scrolled horizontally,
- vertical split secondary pane,
- horizontal split secondary pane,
- terminal resize.

The overlay rectangle must remain in frame and adjacent to the mapped caret/anchor when visible.

Document `LogicalPosition.line`/`character` must not be used directly as global framebuffer x/y.

## 18. Grapheme-aware chrome writer gate

Pass ASCII, CJK, emoji user/data text, and combining sequences through every root/application overlay writer.

Verify:

- no wide grapheme overwrites a border,
- no combining sequence consumes an extra column,
- truncation is cell-width based,
- all written cells remain inside the allocated rectangle.

Application chrome constants remain subject to the stricter ASCII/Box Drawing policy; Unicode data content remains supported.


## 19. Grapheme deletion gate

Use fixtures containing:

- base letter + combining mark,
- emoji represented by multiple code points,
- CJK,
- ordinary ASCII.

Verify plain Backspace and plain Delete remove one complete user-visible grapheme and never leave an invalid partial grapheme sequence due to a one-character forward delete.

A production root path constructing forward deletion as `CharacterOffset(current + 1)` fails the architectural regression check.

## 20. Effective indentation configuration gate

Run navigation/formatting tests with at least:

- tab width 2 + insert spaces true,
- tab width 8 + insert spaces false.

Verify:

- vertical preferred display column uses the configured tab width,
- indent/outdent uses the configured style,
- LSP formatting request carries the configured `tabSize` and `insertSpaces`.

A literal formatting/movement tab size `4` in a path that should use active document settings fails acceptance.

## 21. Language document-identity gate

With two distinct open document IDs, inject inlay-hint/contextual LSP results for the second document.

Verify:

- projected hints retain the second document ID,
- only the owning pane/document paints them,
- a result with a stale version is rejected,
- switching tabs/panes does not cause hints from one document to appear on another.

A fixed `DocumentId(1)` in document-scoped language-result construction fails the regression check.

## 22. Contextual-language presentation gate

Trigger completion, hover, signature help, code action, and rename in automated root tests.

Verify:

- the contextual overlay state opens,
- the generic bottom panel does not switch to a Language-only view solely for those features,
- Escape dismisses the overlay without changing Problems/Output/Search/Git panel selection,
- selected completion/code-action payload still executes correctly.

A parallel fallback that requires `BottomPanelView::Language` for normal contextual operation fails acceptance.

## 23. Audit update

Update `docs/testing/requirements-audit.md` with a section for Requirements 42–51.

For every audit finding A-01 through A-27, record:

- requirement that fixes it,
- automated test that proves it,
- passing status.

Do not mark a finding fixed solely because code was modified.

## Completion criteria

The UI/editor stabilization goal is complete only when:

- Requirements 43–50 are integrated on `devenv`,
- every gate in this document passes,
- A-01 through A-27 each have passing automated proof,
- the worktree is clean,
- no production placeholder/TODO was introduced for a required behavior,
- no old fixed-coordinate or glyph-overwriting fallback remains.

A UI that merely looks correct in one terminal size is not accepted.
