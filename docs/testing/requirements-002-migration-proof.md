# Requirements 002 migration proof

- Branch: `devenv`
- Pre-migration HEAD: `fe94dc8a40eed0c2245c36181bef68753b009cbd`
- Safety branch: `backup/requirements-001-blocked-fe94dc8`
- Canonical checker: `python tools/requirements-002/check_preferred_column_vectors.py` — exit 0; all four supplied vectors passed.
- Authorized adapter: `tests/requirements_001/navigation.rs` now loads `Requirements/002/vectors/preferred-column.json` and executes K004, K005, and both K006 variants through `TextBuffer` vertical movement.
- Non-target acceptance proof: `cargo test --test requirements_001 --no-fail-fast -- --skip k004_preferred_column_empty --skip k005_preferred_column_short --skip k006_preferred_column_tabs` — 75 passed, 0 failed, 3 filtered.
- Full acceptance proof after migration: `cargo test --test requirements_001 --no-fail-fast` — 78 passed, 0 failed.

No production editor behavior was changed for this reconciliation.
