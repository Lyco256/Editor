# 012 — final independent reviews

Run two fresh read-only reviewers.

## Oracle reviewer

Output `docs/testing/requirements-002-final-oracle-review.md`. Verify vectors/checker/adapter unchanged, both K006 variants execute, 78 case IDs exist, no skip/ignore, no fixture-specific production bypass, verifier runs all gates. End `CONFLICTS: N`. Required: 0.

## Product reviewer

Output `docs/testing/requirements-002-final-product-review.md`. Trace and probe resize, keyboard cursor movement, preferred column, click EOL/EOF, selection/caret readability, workbench/menu, split input, tabs/Explorer, search/Git/LSP projection. Any reproducible 001/002 violation is a blocker. End `BLOCKERS: N`. Required: 0.
