# 040 — Edit style top menubar

Owner: WORKBENCH worker

Writable paths:
- `crates/app-ui/src/shell/menu.rs`
- matching docs/tests

## Objective

Add a persistent Edit-like menu bar with Editor's required categories and command-registry actions.

## Required implementation

Render one top row:
`File  Edit  Selection  View  Go  Help`.

Implement:
- F10 focus;
- Alt+F/E/S/V/G/H where modifier is reported;
- Left/Right category;
- Up/Down item;
- Enter execute enabled command;
- Escape close;
- mouse category/item;
- click outside close.

Menu items bind command IDs, not duplicate command logic.

Normal widths >=48 show categories; smaller widths use one `Menu` entry opening the same hierarchy.

No icon-only menu entries.

## Required tests

- `U001_TOP_MENU`
- `U002_MENU_KEYBOARD`
- `U003_MENU_MOUSE`
- resize with menu open
- disabled command cannot execute

## Done only when

menu is discoverable, keyboard/mouse operable, and geometry participates in the authoritative layout snapshot.
