# 064 — projection correctness integration suite

Owner: top Codex after tasks 050–053

Writable paths:
- non-frozen integration tests
- docs/testing evidence

## Objective

Test search/Git/LSP/split projections with exact adversarial fixtures.

## Required implementation

Include:
- duplicate search matches same line;
- dirty-buffer stale hit;
- one-line Git edit in 100 lines;
- add/delete replacement hunks;
- two documents with different diagnostics/semantic/inlay/search;
- LSP overlay after vertical/horizontal scroll;
- Explorer toggle;
- secondary split;
- resize;
- offscreen stale overlay.

Assert exact markers, pane identity, and overlay rectangle.

## Required tests

all `P*` frozen cases at integration level

## Done only when

projection bugs cannot hide behind unit-only helper tests.
