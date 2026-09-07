# 061 — keyboard navigation conformance suite

Owner: top Codex after CORE merge

Writable paths:
- non-frozen integration tests
- docs/testing evidence

## Objective

Exercise the full keyboard routing from normalized KeyEvent through AppState into editor-core, not only editor-core unit methods.

## Required implementation

Create table-driven integration tests for every K001–K023 case.

For each row assert:
- resulting text;
- exact selections;
- preferred-column behavior when applicable;
- viewport scroll intent/reveal;
- undo/redo for mutating commands.

Run at tab widths 2 and 8 for relevant rows.

Include multi-cursor cases.

## Required tests

all `K*` frozen cases plus route-level integration evidence

## Done only when

every required keyboard behavior is reachable from the actual runtime input path and none are silent no-ops.
