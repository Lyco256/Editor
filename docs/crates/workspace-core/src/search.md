# Streaming search and replacement planning

Role: streams project-wide search results, prefers `rg` when available, falls back to Rust search
when it is not, and turns matches into replacement plans before any write begins.

Important types:

- `SearchOptions`, `SearchBackendPreference`, and `SearchSession` control the search lifecycle.
- `SearchHit` carries a matched path, line, and byte range.
- `ReplacementEdit`, `ReplacementPlan`, and `ReplacementReport` describe multi-file replacement.
- `SearchEvent` is the streaming progress/result channel.

Invariants:

- Search results stream through a bounded 256-event channel (backpressure) and can be cancelled.
- Literal, regex, case-sensitive, case-insensitive, whole-word, include, and exclude options are
  supported in both backends.
- Replacement plans group edits per file before any write occurs.
- Replacement byte ranges are checked for bounds, UTF-8 boundaries, and overlap; invalid plans
  return `SearchError::InvalidReplacementRange` instead of being skipped or panicking.

Dependencies:

- `ignore` for fallback discovery and include/exclude filtering.
- `regex` for Rust search matching.
- `document.rs` for atomic save of replacement results.

Tests:

- `rg` JSON fixture parsing.
- fallback literal search.
- replacement grouping.
