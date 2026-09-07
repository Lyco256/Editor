# 072 — geometry and input adversarial review

Owner: fresh read-only reviewer agent

Writable paths:
- no production writes
- report only: `docs/testing/requirements-001-review-geometry.md`

## Objective

Hunt for geometry/input paths that can make resize or mouse behavior appear correct in tests but fail in real layouts.

## Required implementation

Search for:
- hard-coded workbench dimensions/offsets;
- independently recalculated layout;
- stale layout snapshot use;
- fixed tab widths;
- raw screen -> doc arithmetic in AppState;
- dropped wheel/resize events;
- zero-size resize mishandling;
- pane ID assumptions;
- `.chars().count()` used as terminal width;
- wide-cell continuation targeting.

Trace at least one event end-to-end: raw mouse -> normalized event -> layout target -> AppState -> cursor.

End `BLOCKERS: N`.

## Required tests

No code changes.

## Done only when

every remaining geometry/input weakness is reported; final accepted report is `BLOCKERS: 0`.
