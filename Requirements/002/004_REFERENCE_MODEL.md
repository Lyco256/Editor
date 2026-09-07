# 004 — independent reference model

`tools/requirements-002/check_preferred_column_vectors.py` is supplied by this requirement cycle and is not authored by Setup Codex.

It independently calculates line starts, display columns, tab expansion, vertical clamping, and preferred-column retention without importing Rust production code or the Rust acceptance adapter.

Before the acceptance adapter is changed, run it and require exit 0. If it fails, setup stops with `ORACLE_STATUS = INVALID` and no baseline is frozen.

The checker becomes frozen in the 002 baseline and Goal Mode cannot edit it.
