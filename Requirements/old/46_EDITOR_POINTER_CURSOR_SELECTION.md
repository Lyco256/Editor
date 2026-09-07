# Subagent requirement — editor pointer, caret, and selection behavior

## Branch

`feat/editor-pointer-selection`

## Wave

Wave 4B. Start only after Wave 4A is integrated and green.

## Writable ownership

- `crates/app-ui/src/editor/pointer.rs`
- `crates/app-ui/src/editor/cursor.rs`
- `crates/app-ui/src/editor/selection.rs`
- matching mirrored docs
- pointer/cursor fixtures and snapshots assigned by the top foundation

Do not edit root `src/app/**`, editor facade/viewport, editor-core, shell modules, shared types, terminal backend, or `Cargo.lock`.

## Goal

Define deterministic pane-local pointer mapping and non-destructive caret/selection rendering suitable for top-runtime integration.

## 1. Text-area click mapping

The pointer mapper receives:

- actual pane text rectangle dimensions,
- pane viewport top line/left display column,
- snapshot,
- tab width,
- pane-local pointer coordinate.

It never receives or subtracts Explorer/menu/status global offsets.

### Horizontal clamp

For a pointer row that maps to an existing logical line:

- click on a source grapheme -> caret at that grapheme boundary,
- click at or to the right of the rendered line end -> caret at logical line end,
- click within blank space after EOL -> line end,
- click left of the first visible source column while horizontally scrolled -> nearest visible valid document boundary.

### Vertical clamp

- click on an existing blank logical line -> that line's EOL,
- click below the last logical document line but still inside the editor text rectangle -> EOF,
- click below visible source content in a short file -> EOF,
- no click inside the editor text rectangle returns `None` merely because it is beyond text length.

## 2. Drag mapping

Left-button drag clamps using the same horizontal/vertical rules as click.

Dragging beyond:

- EOL extends to EOL,
- the last document line extends to EOF,
- the first visible line upward clamps to the nearest reachable document position and cooperates with top-level autoscroll routing.

A drag never generates an invalid character offset or points into a grapheme.

## 3. Click gestures

Pure typed actions are exposed for:

- single click -> place primary caret/collapse selection,
- Shift+single click -> extend primary selection from its anchor,
- Alt+single click -> add a secondary cursor at mapped position,
- double click -> select current word,
- triple click -> select entire logical line including line ending when present.

The word-selection algorithm consumes the effective word-boundary policy supplied by root/language configuration.

## 4. Gutter pointer semantics

The mapper distinguishes:

- line-number region,
- fold-marker region,
- source text region,
- overview ruler.

Required behavior:

- line-number single click selects the logical line,
- line-number drag extends by whole lines,
- fold-marker click toggles the fold at that line,
- overview-ruler click does not move the text caret directly; it emits a scroll/reveal request.

## 5. Primary caret rendering

The primary focused-editor caret is not drawn by replacing a framebuffer source glyph.

The cursor module returns a terminal cursor presentation for the active primary selection:

- visible only when the focused editor has a visible active cursor,
- screen position computed from actual text rectangle + display column - horizontal viewport,
- shape `SteadyBar`.

The source framebuffer cell remains the original source grapheme.

When a menu/modal/picker has keyboard focus, the editor hardware caret is hidden.

## 6. Selection rendering

Selected source cells retain the original source grapheme.

Selection styling uses:

- `SelectionForeground`,
- `SelectionBackground`.

A selected cell must never be replaced with `▌`, block glyphs, spaces, or decorative cursor symbols.

## 7. Secondary cursors

A terminal can expose only one hardware cursor.

Secondary cursors are represented without replacing the underlying glyph:

- retain original grapheme,
- apply `SecondaryCursorForeground` / `SecondaryCursorBackground`,
- at EOL, style exactly one blank cell when space is available.

Primary caret shape and secondary-cursor style remain visually distinct.

## 8. Wide/combining graphemes

Caret and selection positions use display-cell ownership.

A caret never targets the continuation half of a wide grapheme.

A combining sequence is treated as one grapheme.

## 9. Wheel routing primitive

Pointer UI exposes typed pane-local wheel actions.

Mouse wheel over an editor pane scrolls that pane by the normalized `ScrollLines` delta while preserving the cursor location unless cursor reveal policy is explicitly triggered by a later keyboard movement.

Wheel input over Explorer, panel, or menu is not consumed as editor scroll.

## Tests

Tests cover:

- click exact source cell,
- click right of EOL -> EOL,
- click far right -> EOL,
- click an empty line,
- click below EOF -> EOF,
- drag after EOL,
- drag below EOF,
- Shift-click selection extension,
- Alt-click secondary cursor action,
- double-click word,
- triple-click logical line,
- line-number selection,
- fold marker hit,
- overview click emits reveal/scroll action,
- source grapheme unchanged under primary caret,
- source grapheme unchanged under selection,
- source grapheme unchanged under secondary cursor,
- EOL secondary cursor,
- CJK wide grapheme,
- combining grapheme,
- horizontally scrolled click,
- wheel event bound to exact pane.

## Acceptance criteria

- pointer mapping contains no global shell magic numbers,
- empty/right/below-text clicks always clamp to a useful valid document position,
- primary caret uses native cursor presentation,
- selected text remains readable,
- secondary cursors preserve underlying source symbols,
- all owned tests, docs, format, and Clippy checks pass.
