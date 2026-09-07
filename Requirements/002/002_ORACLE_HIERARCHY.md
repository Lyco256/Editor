# 002 — oracle hierarchy

Precedence is fixed:

1. requirement-authored canonical machine-readable vectors for exact deterministic fixtures/checkpoints;
2. written behavioral invariant explaining the vector;
3. Rust acceptance adapter that loads the vector and executes production code;
4. existing production unit tests as supporting evidence.

The Rust adapter is never a source of expected values.

Before Goal, any disagreement among these levels sets `ORACLE_STATUS = INVALID`; setup must not freeze and Goal must not start. During Goal, levels 1–3 are immutable. A difficult product failure is not an oracle conflict merely because implementation is hard.
