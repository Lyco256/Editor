# 013 — persistent pane view state

Owner: top Codex

Writable paths:
- `src/app/state.rs`
- `src/app/scene.rs`
- matching docs

## Objective

Persist viewport, fold, focus, and displayed-document state per pane exactly as interactive textarea state, rather than recreating frame-local defaults.

## Required implementation

1. Every pane has stable ID, displayed tab/document, vertical scroll origin, horizontal scroll origin, fold/collapse state, and focus state.
2. Scene projection reads these fields; it does not call `TextViewport::default()` as an ordinary fallback for an existing pane.
3. A missing pane state is created when the pane is created, not during rendering.
4. Manual wheel scroll updates only the target pane.
5. Cursor-reveal logic updates only focused pane and only when requested by editor movement/editing.
6. Fold state survives syntax refresh when fold region identity remains valid.
7. Same document may appear in two panes sharing text buffer while retaining independent viewport/folds.

## Required tests

- `P002_PANE_VIEW_STATE_PERSISTENT`
- same-document/two-pane independent-scroll unit test
- syntax refresh retains collapsed fold test

## Done only when

pane visual state survives arbitrary renders and is never regenerated from defaults during scene construction.
