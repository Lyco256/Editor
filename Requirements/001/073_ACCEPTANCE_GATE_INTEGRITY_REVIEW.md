# 073 — acceptance gate integrity review

Owner: fresh read-only reviewer agent

Writable paths:
- no production writes
- report only: `docs/testing/requirements-001-review-gate.md`

## Objective

Prove the Goal cannot pass by weakening, skipping, or bypassing the frozen oracle.

## Required implementation

1. Validate `BASELINE_REF`.
2. Diff every frozen path.
3. Compare case IDs in requirement 001, required-cases manifest, and actual test list.
4. Search for ignore/cfg/early-return skip patterns.
5. Inspect verifier scripts for omitted commands or `|| true`/error suppression.
6. Verify acceptance tests call real production layout/navigation/input paths rather than replacement mocks.
7. Verify reviewer-zero check is enforced.
8. End `BLOCKERS: N`.

## Required tests

No code changes.

## Done only when

the gate is tamper-evident and all cases are actually executed; final accepted report is `BLOCKERS: 0`.
