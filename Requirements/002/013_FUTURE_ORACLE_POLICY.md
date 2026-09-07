# 013 — mandatory policy for future requirement cycles

This is the permanent prevention rule.

For deterministic behavior, the requirement author supplies canonical input/expected-state data before Setup. Setup Codex may create plumbing but may not invent fixture text, starting positions, operation sequences, or expected results.

When practical, each vector family gets a small independent reference model/checker that does not call production code. Two fresh reviewers validate spec/vector and vector/adapter before freeze.

For non-vectorizable behavior, freeze a machine-checkable invariant instead, such as rectangle containment or render/hit identity. Do not freeze vague assertions like “works”, “does not panic”, or “looks correct”.

No Goal baseline is valid until all oracle consistency checks are green. Goal Mode is for satisfying an already-valid oracle, not for discovering whether its tests mean the same thing as its requirements.
