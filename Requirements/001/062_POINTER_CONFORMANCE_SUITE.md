# 062 — pointer conformance suite

Owner: top Codex after POINTER merge

Writable paths:
- non-frozen integration tests
- docs/testing evidence

## Objective

Exercise raw screen mouse event -> layout snapshot -> typed target -> document action end-to-end.

## Required implementation

For representative 80x24 and 120x40 scenes and after resize:
- exact text click;
- EOL/right whitespace;
- blank line;
- below EOF;
- Shift click;
- Alt click;
- drag;
- double/triple click;
- wheel;
- split pane click;
- Explorer click.

Assert resulting pane focus, selection/cursor, and viewport.

## Required tests

all `M*` frozen cases at root integration level

## Done only when

mouse correctness is proven through actual rendered geometry and application state, not a pointer helper alone.
