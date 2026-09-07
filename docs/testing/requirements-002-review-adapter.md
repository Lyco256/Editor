# Requirements/002 adapter review

Reviewer B performed a read-only review of the K004/K005/K006 acceptance adapter on `devenv`.

## Scope and oracle wiring

- `tests/requirements_001/navigation.rs` is the only acceptance adapter file changed for the
  migration. The K004, K005, and K006 test bodies select their case IDs from the canonical vector
  file; they no longer contain the superseded fixture/expected-value pairs.
- The adapter resolves `Requirements/002/vectors/preferred-column.json` from
  `CARGO_MANIFEST_DIR`, so it does not depend on the process working directory.
- Vector data supplies the document, tab width, initial offset, operations, and checkpoint values.
  The adapter constructs a real `TextBuffer`, sets the initial selection, invokes
  `move_vertical` for each canonical `down`, and checks offset, logical line/character, display
  column, and stored preferred display column after every operation.
- K006 selects both supplied variants (`tab-width-4` and `tab-width-8`) through the same case-ID
  adapter path.

## Independent execution evidence

- `python tools/requirements-002/check_preferred_column_vectors.py`: passed all four supplied
  vectors (K004, K005, K006 width 4, K006 width 8).
- `cargo test --test requirements_001 --no-fail-fast`: 78 passed, 0 failed, 0 ignored.
- `cargo test --test requirements_001 --no-fail-fast -- --skip k004_preferred_column_empty
  --skip k005_preferred_column_short --skip k006_preferred_column_tabs`: 75 passed, 0 failed,
  3 filtered. This demonstrates the non-target cases remain passing.

## Static preservation checks

- All 78 IDs in `tools/requirements-001/required-cases.txt` have corresponding test functions.
- No `#[ignore]`, `#[cfg(...)]`, or disabled required-case annotations occur in
  `tests/requirements_001/*.rs`.
- The active K004/K005/K006 adapter contains none of the old K004 empty-line fixture, old K004
  expected offset, old K005 expected offset, old K006 tab fixture, or old K006 expected offset.
- The working-tree production-side diff is limited to a read-only preferred-column observation
  accessor and its required Markdown mirror; it introduces no case-ID, fixture, test-environment,
  or process-specific production branch and does not alter editor behavior.

The adapter review found no oracle conflict or acceptance weakening.

CONFLICTS: 0
