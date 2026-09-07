# Subagent requirement — Edit-like workbench and menu bar

## Branch

`feat/workbench-menubar`

## Writable ownership

- `crates/app-ui/src/shell/workbench.rs`
- `crates/app-ui/src/shell/menu.rs`
- matching mirrored docs
- workbench/menu fixtures and snapshots assigned by the top foundation

Do not edit root `src/app/**`, shell facade/geometry, Explorer/tabs/panes files, shared types, terminal backend, or `Cargo.lock`.

## Goal

Give Editor an immediately understandable top-level layout with Microsoft Edit as the minimum usability baseline and VS Code as the workbench organization reference.

The result remains terminal-native and dense rather than attempting to imitate GUI pixels.

## 1. Normal workbench layout

For a normal terminal size, the vertical order is:

1. one-row menu bar across the full width,
2. workbench main area,
3. one-row status bar across the full width.

Inside the main area:

- Explorer occupies the left side when visible,
- one or more editor groups occupy the right side,
- the bottom panel is below the editor-group area only,
- each editor group owns its own tab strip,
- editor text occupies all remaining group content space.

There is no global tab strip spanning across Explorer.

## 2. Information-density rules

Normal editor layout consumes only:

- 1 row menu bar,
- 1 row tab strip per visible editor group,
- 1 row status bar,
- 1 cell per split separator,
- 1 cell overview ruler where enabled,
- dynamic line-number/gutter width.

Do not draw an outer box around every editor pane.

Do not reserve a large decorative right-side area.

Do not fill unused areas with repeated horizontal rules.

A horizontal split may use exactly one separator row. A vertical split may use exactly one separator column.

## 3. Menu categories

The normal menu bar contains exactly these top-level categories in this order:

`File  Edit  Selection  View  Go  Help`

These labels are ASCII.

### File

Provide command entries for:

- New File
- Open File
- Open Folder
- Add Folder to Workspace
- Open Recent
- Save
- Save As
- Reopen with Encoding
- Save with Encoding
- Change End of Line Sequence
- Close Editor
- Close Other Editors
- Exit

### Edit

Provide:

- Undo
- Redo
- Cut
- Copy
- Paste
- Find
- Replace
- Find in Files
- Replace in Files

### Selection

Provide:

- Select All
- Select Line
- Expand Selection
- Add Cursor Above
- Add Cursor Below
- Add Selection to Next Find Match
- Skip Current Selection and Select Next Match
- Select All Occurrences
- Collapse to Single Cursor

### View

Provide:

- Command Palette
- Toggle Explorer
- Problems
- Output
- Source Control
- Split Editor Right
- Split Editor Down
- Close Editor Group
- Focus Left Group
- Focus Right Group
- Focus Group Above
- Focus Group Below
- Toggle Line Numbers

### Go

Provide:

- Go to Line/Column
- Go to Definition
- Go to Declaration
- Go to Implementation
- Find References
- Next Problem
- Previous Problem

### Help

Provide:

- Show Keyboard Shortcuts
- About Editor

If a command is unavailable in the current state, its menu row is visibly disabled and cannot execute.

## 4. Menu interaction

Required keyboard interaction:

- `F10` focuses the menu bar,
- `Alt+F/E/S/V/G/H` opens the corresponding menu where the terminal reports the modifier,
- Left/Right changes the active top-level menu,
- Up/Down changes selected menu item,
- Enter executes,
- Escape closes one menu level and returns focus predictably,
- accelerator letters may be shown with a terminal-supported underline or high-contrast fallback.

Required mouse interaction:

- click category opens it,
- click another category switches directly,
- click command executes when enabled,
- click outside closes the open menu.

Menu interaction uses command IDs from the authoritative command registry; the menu does not duplicate application command logic.

## 5. Compact behavior

At width >= 48 cells and height >= 4 rows, the full top-level menu labels remain visible.

Below 48 cells, replace the category row with a one-cell-high `Menu` entry that opens the same command hierarchy.

At extremely small heights where preserving the editor requires collapsing chrome, the existing editor-first compact policy may hide menu/status rows, but this state is tested explicitly and must never corrupt geometry.

## 6. Edit visual influence

Keep the Edit-like characteristics:

- immediate visible menu discoverability,
- simple rectangular menus,
- dense one-row chrome,
- keyboard focus,
- mouse support,
- no icon-only controls.

Do not copy Microsoft Edit source layout blindly where it would remove required VS Code-like Explorer/editor-group functionality.

## Tests

Framebuffer/state tests cover:

- 120x40 standard layout,
- 80x24 standard layout,
- width 47 compact menu,
- tiny editor-first layout,
- File/Edit/Selection/View/Go/Help labels,
- F10 menu focus,
- Alt access keys,
- keyboard menu traversal,
- disabled command,
- mouse category switch,
- outside-click dismissal,
- no outer editor-pane border,
- no repeated filler-rule region.

## Acceptance criteria

- normal workbench has the required menu bar,
- editor-group tabs no longer consume the Explorer top row,
- workbench chrome is information-dense,
- every menu item is backed by a command-registry ID,
- menus work by keyboard and mouse in pure tests,
- all owned tests, docs, format, and Clippy checks pass.
