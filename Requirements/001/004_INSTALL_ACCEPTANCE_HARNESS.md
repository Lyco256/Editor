# 004 — install the acceptance harness before fixes

Owner: top Codex.
Mode: setup run only.

Do not modify production behavior in this task.

## Create frozen acceptance test area

Create:

- `tests/requirements_001/`
- `tests/requirements_001/support/`
- `tools/requirements-001/required-cases.txt`
- `tools/verify-001.ps1`
- `tools/verify-001.sh`

The exact test-file split is:

- `tests/requirements_001/resize_geometry.rs`
- `tests/requirements_001/navigation.rs`
- `tests/requirements_001/pointer_caret.rs`
- `tests/requirements_001/workbench.rs`
- `tests/requirements_001/projection.rs`
- `tests/requirements_001/robustness.rs`
- `tests/requirements_001/support/mod.rs`

If root integration tests cannot access private APIs, add only test seams required to observe behavior. Test seams must not change production semantics.

## Required-case manifest

`required-cases.txt` contains every case ID from requirement 001, one ID per line, in the same order.

No extra replacement aliases.

## Test naming

Every acceptance test function includes its case ID in the Rust test name.

Example convention:

`r001_resize_immediate`

The support layer may create fake terminals, fake action sources, temporary files, and deterministic clocks.

## Verifier responsibilities

Both verifier scripts perform the same logical checks and return non-zero on any failure.

They:

1. read `Requirements/001/BASELINE_REF.txt`;
2. verify it contains one valid commit;
3. diff frozen paths against that commit and require no changes;
4. require all case IDs in the manifest;
5. scan frozen acceptance sources for `#[ignore]`, conditional disabling of individual acceptance cases, or renamed/missing IDs;
6. run format check;
7. run workspace Clippy with warnings denied;
8. run normal workspace tests;
9. run all acceptance tests;
10. run docs mirror verification;
11. run forbidden-pattern checks specified by requirement 075;
12. verify the four reviewer reports exist and contain `BLOCKERS: 0` in the final phase.

During the setup phase, the scripts may support a `--pre-goal` option that skips baseline/reviewer-final checks but still runs/list-checks acceptance tests.

## Initial red-state proof

After adding the acceptance tests, run the 001 suite against the current unmodified production implementation.

Write `docs/testing/requirements-001-baseline-failures.md` containing:

- exact command;
- exit status;
- failing case IDs;
- concise reason for each known failure.

At minimum the setup must observe failures for the known current defects, including resize, fixed 120x40 hit testing, preferred-column preservation, and source-glyph-overwriting caret.

If all such known-broken cases unexpectedly pass, stop setup and inspect the test harness; do not freeze a false oracle.

## Existing-test protection

Before freezing, run the repository's existing non-001 test suite and record its status.

Acceptance test installation itself must not break production compilation except where a 001 test intentionally references the specified missing behavior/API.
