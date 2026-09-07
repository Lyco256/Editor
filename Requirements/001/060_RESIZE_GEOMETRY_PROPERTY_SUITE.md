# 060 — resize geometry property suite

Owner: top Codex after merges

Writable paths:
- non-frozen supporting tests outside frozen acceptance paths
- docs/testing evidence

## Objective

Broaden resize/layout verification beyond fixed examples.

## Required implementation

Generate valid terminal sizes in a bounded range including widths 20–220 and heights 5–60.

For each:
- render scene;
- assert all rects contained;
- assert non-overlay region overlap rules;
- assert status bottom when visible;
- assert at least focused editor remains when compact policy permits;
- for each visible identity rect, hit its center and assert same identity;
- resize to another generated size and ensure old snapshot is not used.

Include shrink/expand sequences.

## Required tests

- frozen `Q002_RESIZE_PROPERTY`
- proptest/randomized deterministic seed test

## Done only when

geometry invariants hold across generated sizes, not only snapshots.
