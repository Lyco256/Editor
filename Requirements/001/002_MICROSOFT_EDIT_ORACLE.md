# 002 — Microsoft Edit behavioral oracle

Repository: `https://github.com/microsoft/edit`
License: MIT.

For the overlap below, Editor reproduces the behavior, not merely the visual appearance.

## Reference source

Primary source file:

- `crates/edit/src/tui.rs`

Additional:

- `crates/edit/src/bin/edit/draw_menubar.rs`
- `crates/edit/src/bin/edit/draw_editor.rs`

## Oracle rules

### Resize

Microsoft Edit keeps terminal size as TUI state.

When it receives `Input::Resize`, it validates the size and immediately replaces stored TUI size. The next layout/render uses the new `size.as_rect()`.

Editor must do the same at its runtime boundary. Do not forward Resize to an editor state branch that returns success without changing render size.

### Render geometry is also hit-test geometry

Microsoft Edit retains the previously rendered UI tree. Mouse targeting walks the prior tree's actual clipped rectangles.

Editor must use the same principle: the layout snapshot produced for the presented frame is the only geometry source for the next pointer event.

Do not regenerate a guessed 120x40 layout or subtract hard-coded shell widths.

### Persistent textarea view state

Microsoft Edit's textarea carries scroll offset and preferred column across frames by recovering them from the previous UI node.

Editor must preserve:

- vertical viewport,
- horizontal viewport,
- preferred vertical column,
- fold state,
- focus/pane identity.

They are not frame-local defaults.

### Mouse selection

Edit derives mouse document position from the textarea's actual inner rectangle, margin width, current scroll, and buffer visual coordinates.

Required overlap:

- normal click places cursor,
- Shift+click extends selection,
- drag extends selection,
- click in left/gutter selection region selects line,
- double click selects word,
- triple click selects line,
- dragging near/outside textarea participates in scrolling rather than producing invalid positions.

Editor additionally supports Alt+click multiple cursors.

### Preferred column

Edit stores `preferred_column`.

Vertical movement uses that stored goal. It is not recomputed from a cursor clamped to a short line.

Edit changes preferred column after non-vertical movement; prior/next/up/down preserve it.

Editor must reproduce this core rule, per cursor.

### Left/Right

Edit uses grapheme movement. Without Shift, a non-empty selection collapses toward beginning/end before further motion.

Editor must reproduce this.

### Home/End

Edit Home supports smart indentation behavior: first useful home position is indentation/start-of-text, repeated Home can move to true line start.

Ctrl+Home goes buffer start.

End goes line end; Ctrl+End goes buffer end.

Editor initial no-word-wrap implementation uses the corresponding logical-line behavior.

### PageUp/PageDown

Edit moves using viewport height and preferred column, with Shift extension.

Editor implements the same principle using the focused pane's current visible text height.

### Ctrl+Up/Ctrl+Down

Edit can scroll the viewport without moving the text cursor.

Editor reproduces this.

### Delete/Backspace

Edit deletes by grapheme for ordinary Delete/Backspace and by word for modified deletion.

Editor must never implement forward Delete as `current character offset + 1`.

### Tab/ShiftTab

Edit supports indentation/unindentation behavior.

Editor applies effective `.editorconfig`/workspace indentation settings.

### Cursor reveal

Edit scrolls just enough to reveal active cursor rather than resetting viewport wholesale.

Editor follows the same rule. Mouse-wheel scroll is allowed to leave cursor off-center until a cursor-moving/editing action requests reveal.

### Menubar

Edit has a persistent top menu bar and keyboard focus with F10.

Editor uses Edit's discoverability baseline and adds required VS Code categories:

`File Edit Selection View Go Help`.

## Non-oracle Editor extensions

Do not attempt to copy Edit for features Edit does not define sufficiently:

- multiple cursors,
- editor groups/splits,
- Preview/Open/Pinned tab state,
- Explorer workspace tree,
- LSP overlays,
- Git gutters,
- project search.

Those behaviors are fully specified elsewhere in Requirements 001.
