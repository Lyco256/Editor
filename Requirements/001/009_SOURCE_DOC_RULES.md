# 009 — source documentation rule for this cycle

The existing source-to-doc mirror rule remains active.

For every new or moved production Rust source file, create/update the matching `docs/.../*.md`.

Each source doc states:

- responsibility;
- authoritative state owned by the module;
- coordinate systems accepted/returned;
- invariants;
- dependencies;
- error/fallback behavior;
- tests that prove the behavior.

For layout/input modules, explicitly state that screen cells, display columns, logical positions, character offsets, and byte offsets are distinct coordinate domains.

Do not copy source code line-by-line into docs.
