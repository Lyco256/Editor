# Subagent requirement — VS Code-like editor navigation semantics

## Branch

`feat/editor-navigation`

## Writable ownership

- `crates/editor-core/**`
- `docs/crates/editor-core/**`
- editor-core navigation fixtures/property tests

Do not edit root `src/app/**`, app-ui, shared types, terminal backend, or `Cargo.lock`.

## Goal

Bring basic text editing/navigation behavior to the level expected from a mature conventional editor, using VS Code behavior as the primary interaction reference while respecting terminal input limits.

## 1. Preferred display column

Each active selection/cursor has persistent vertical-navigation goal state.

For consecutive vertical moves:

1. if no preferred display column exists, capture the current display column,
2. move to the requested target line,
3. clamp to that line's nearest valid position when the line is shorter,
4. preserve the original preferred display column,
5. when a later line is long enough, return to that preferred display column.

The goal is a display-cell column, not a byte offset or Unicode scalar count.

Tabs and wide graphemes are included in display-column calculation.

Each cursor in a multi-cursor selection set retains its own preferred display column.

## 2. Preferred-column reset rules

Reset/recompute the affected cursor's preferred column after:

- Left/Right movement,
- Home/End,
- direct mouse placement supplied by root,
- explicit selection replacement,
- text insertion/deletion that moves the active caret,
- word navigation,
- document start/end navigation.

Do not reset it merely because Up/Down/PageUp/PageDown clamped on a short line.

Shift+vertical selection extension preserves the same goal.

## 3. Required regression example

The following is mandatory:

Document:

`abcde\n\nabcde`

Start at line 1 display column 5.

- Down -> line 2 column 0, preferred column remains 5.
- Down -> line 3 column 5.
- Up -> line 2 column 0, preferred remains 5.
- Up -> line 1 column 5.

The same test exists with tabs and a full-width CJK grapheme before the target display column.

## 4. Horizontal movement

Left/Right move by safe user-visible text boundary and never split a grapheme.

When a non-empty selection exists and Shift is not held:

- Left collapses to selection start,
- Right collapses to selection end.

Shift+Left/Right extends the selection.

## 5. Home/End

Home uses smart-home behavior:

- first press moves to first non-whitespace position on the logical line,
- if already there, next press moves to absolute line start,
- subsequent presses toggle between those two positions.

End moves to logical line end.

Shift variants extend the selection.

## 6. Document navigation

Required commands:

- Ctrl+Home -> document start,
- Ctrl+End -> document end,
- Shift+Ctrl+Home/End -> extend to document start/end.

## 7. Word navigation/deletion

Use caller-supplied language/word-boundary configuration with a deterministic fallback.

Required:

- Ctrl+Left -> previous word boundary,
- Ctrl+Right -> next word boundary,
- Shift variants extend,
- Ctrl+Backspace -> delete to previous word boundary,
- Ctrl+Delete -> delete to next word boundary.

All operate safely on Unicode text.


## 8. Forward Delete and Backspace grapheme semantics

Plain Backspace deletes the previous complete grapheme cluster when the selection is empty.

Plain Delete deletes the next complete grapheme cluster when the selection is empty.

For a non-empty selection, Backspace and Delete both delete the selected range.

A single user command is one transaction.

A base character plus combining marks, emoji sequence, or other multi-code-point grapheme must never be left partially deleted by either command.

The root runtime must call editor-core operations for both directions; it must not construct `current_offset + 1` forward-delete ranges itself.

## 9. Effective tab/indent settings

All display-column-sensitive editor-core operations accept the effective document tab width supplied by root configuration.

The top integration must use `AppState.tab_width` / effective `.editorconfig` values rather than literal `4`.

Indent/outdent uses the effective `insert_spaces` and tab width.

No independent editor-navigation constant is allowed to disagree with the document's effective indentation configuration.

## 10. Page navigation and scrolling

PageUp/PageDown:

- move cursor approximately one visible editor page supplied by the caller,
- preserve preferred display column,
- extend with Shift.

Ctrl+Up/Ctrl+Down produce viewport-scroll intents without moving the text selection.

The core exposes the distinction; root owns the actual pane viewport.

## 11. Indent/outdent

Tab:

- with a multiline/non-empty selection -> indent affected logical lines,
- at a standalone caret -> insert configured indentation to the next indentation stop.

Shift+Tab outdents affected line(s) without deleting non-indentation content.

Respect tab width and insert-spaces configuration supplied by the caller.

## 12. Line operations

Implement transaction-safe operations for:

- Alt+Up -> move current/selected line block up,
- Alt+Down -> move current/selected line block down,
- Shift+Alt+Up -> copy current/selected line block up,
- Shift+Alt+Down -> copy current/selected line block down,
- Ctrl+Enter -> insert new line below,
- Ctrl+Shift+Enter -> insert new line above,
- Ctrl+L -> select current logical line,
- Ctrl+A -> select entire document.

Operations apply coherently to multi-cursor selections after normalization.

## 13. Undo/redo semantics

Each line operation or multi-cursor command triggered once is one transaction.

Undo restores:

- exact text,
- exact selections,
- selection primary identity,
- navigation goal state required to continue predictable cursor movement.

Redo restores the corresponding post-operation state.

## Property and regression tests

Tests cover:

- required empty-line preferred-column example,
- multiple short lines before a longer line,
- tabs,
- CJK width,
- combining grapheme,
- multiple cursors with different preferred columns,
- smart Home toggle,
- End,
- document start/end,
- word navigation and deletion,
- plain Delete/Backspace on combining graphemes and emoji sequences,
- non-empty selection Delete/Backspace,
- tab width 2/4/8 preferred-column behavior,
- page movement with supplied page height,
- Ctrl+Up/Down emits scroll intent without cursor mutation,
- indent/outdent,
- move/copy lines at document boundaries,
- line insertion above/below,
- Ctrl+L,
- Ctrl+A,
- undo/redo for every mutating operation.

## Acceptance criteria

- vertical movement never loses the desired display column only because an intermediate line is short/empty,
- navigation never creates invalid Unicode boundaries,
- plain forward Delete is grapheme-safe and implemented by editor-core rather than root `offset + 1`,
- all display-column operations consume effective document tab width instead of a hard-coded value,
- core exposes every stated mature-editor operation without UI/terminal dependencies,
- every mutating operation is transactionally undoable,
- all editor-core tests, docs, format, and Clippy checks pass.
