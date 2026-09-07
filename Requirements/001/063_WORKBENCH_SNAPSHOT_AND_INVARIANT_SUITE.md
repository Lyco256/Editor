# 063 — workbench snapshot and invariant suite

Owner: top Codex after WORKBENCH merge

Writable paths:
- non-frozen workbench tests/snapshots
- docs/testing evidence

## Objective

Verify Edit-like workbench structure and density without relying only on visual inspection.

## Required implementation

At 40x10, 48x12, 80x24, 120x40, 160x50:
- snapshot;
- assert menu labels/popup action IDs;
- assert Explorer kind labels;
- tab dispositions;
- one-cell separators;
- no phantom pane;
- no filler ownership region;
- status placement;
- chrome glyph scan;
- contrast palette tests.

For CJK file/tab labels, assert exact cell bounds/hit identities.

## Required tests

all `U*` and `Q001` frozen workbench cases

## Done only when

workbench structure, density, color semantics, and width safety are mechanically checked.
