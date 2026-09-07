# 051 — exact project search markers

Owner: top Codex

Writable paths:
- `src/app/scene.rs`
- `crates/app-ui/src/workspace/**` only if exact range field projection is missing
- matching docs/tests

## Objective

Preserve backend exact search match identity instead of re-searching display text.

## Required implementation

1. Carry `workspace-core::SearchHit.byte_range` through the UI/result model.
2. Carry enough source/file version identity to decide whether the range is still valid for active buffer.
3. For valid clean source, convert exact byte boundaries to current editor character offsets.
4. For dirty/changed buffer, validate; if invalid, omit stale marker until fresh search rather than moving it.
5. Delete runtime use of `line_text.find(matched_text)` as marker location reconstruction.
6. Navigation chooses the exact selected result occurrence.

## Required tests

- `P004_SEARCH_EXACT_RANGE`
- `P005_SEARCH_STALE_DIRTY`
- two equal matches on one line
- Unicode byte/char conversion

## Done only when

each backend search hit remains the same occurrence through rendering/navigation and stale hits are never silently relocated.
