# 044 — split pane geometry

Owner: WORKBENCH worker

Writable paths:
- `crates/app-ui/src/shell/panes.rs`
- matching docs/tests

## Objective

Render nested split panes with exact identities and one-cell separators.

## Required implementation

1. Every leaf has stable pane ID.
2. Every pane owns its local tab strip and editor content rect.
3. Vertical/horizontal split uses stored ratio and one separator.
4. Ratio clamps so both children retain usable editor content when size permits.
5. Focused pane is visually distinguishable through tab/separator/background role, not extra outer border.
6. Tiny-layout collapse preserves focused editor pane.
7. Layout snapshot records all pane/subrect identities.
8. No screen-half assumption.

## Required tests

- `R007_SPLIT_EXACT_HIT`
- `U010_GROUP_LOCAL_TABS`
- vertical/horizontal/nested split layouts
- compact collapse

## Done only when

split geometry is deterministic, pane-ID-based, and contains no fixed 60/20 boundary assumptions.
