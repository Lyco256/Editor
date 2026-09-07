# Requirements 002 canonical-vector review

Read-only review performed against `Requirements/002_README.md` and Requirements 002/000–010.
No production source or acceptance adapter was changed by this review.

## Commands and evidence

- `python tools/requirements-002/check_preferred_column_vectors.py` exited 0 and reported PASS for all four supplied vectors: K004 `empty-middle-line`, K005 `short-middle-line`, and K006 `tab-width-4` plus `tab-width-8`.
- `cargo test -p editor-core vertical_navigation -- --nocapture` exited 0. The existing editor-core preferred-column tests for blank-line retention and per-cursor column retention both passed.
- The supplied checker imports only Python standard-library modules (`pathlib`, `json`, and `sys`); it does not import Rust production code or the Rust acceptance adapter.

## Independent derivation

The vector conventions are zero-based insertion offsets and display-cell columns. For K004, `abcde` has its initial caret at display column 5; Down reaches the empty middle line at its line start (offset 6, logical character 0, display 0) while retaining preferred column 5; the second Down reaches the final `abcde` at offset 12 and display column 5. For K005, the one-character middle line clamps to logical character 1 at offset 7 while retaining preferred column 5; the second Down reaches offset 13, character 5, display 5. These agree with the written ruling and the checker.

For K006, the initial caret is after `\tab` in the first line. A tab advances to the next tab stop, so the initial preferred display column is 6 with width 4 and 10 with width 8. The second line contains ordinary digits, therefore Down lands at logical character/display column 6 (offset 11) for width 4 and character/display column 10 (offset 15) for width 8. The supplied vectors encode both variants and the checker derives the same checkpoints.

## Conflict assessment

The machine-readable vectors, their independent reference model, the written preferred-column invariant, and the existing canonical editor-core navigation tests are consistent. The old K004/K005/K006 frozen fixture values are correctly superseded by the formal 002 ruling; this is an oracle correction, not a product-behavior contradiction.

CONFLICTS: 0
