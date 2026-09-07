# 043 — dense workbench geometry

Owner: WORKBENCH worker

Writable paths:
- `crates/app-ui/src/shell/workbench.rs`
- matching docs/tests

## Objective

Maximize useful terminal information density and remove unexplained borders/filler.

## Required implementation

Normal vertical structure:
1 row menu;
main workbench;
1 row status.

Main:
Explorer left when visible;
editor groups right;
bottom panel below editor-group area, not below Explorer.

Editor group consumes:
1 row local tab strip;
editor content.

Do not draw an outer box around every editor pane.
Do not reserve a decorative right strip.
Overview ruler is one cell.
Vertical split separator is one cell.
Horizontal split separator is one row.
Hidden panel returns all rows.
One group consumes all available editor-group area.

Provide cell-ownership classification for layout property tests.

## Required tests

- `R008_NO_FILLER_REGION`
- `U010_GROUP_LOCAL_TABS`
- `U011_NO_PHANTOM_PANE`
- `U012_DENSE_LAYOUT`
- 40x10, 80x24, 120x40, 160x50 snapshots

## Done only when

normal layout has no phantom pane/filler area and spends only the specified cells on chrome/separators.
