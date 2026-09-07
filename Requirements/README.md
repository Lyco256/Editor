# Editor stabilization — top Codex entry point

All previous requirement files are assumed to have been moved to `Requirements/old/`.

For this stabilization cycle:

- top-Codex instructions live directly under `Requirements/`;
- detailed immutable implementation contracts live under `Requirements/001/`;
- implementation evidence and progress logs are written under `docs/testing/`, not inside `Requirements/001/`.

## Mandatory read order

Before changing production code, the top Codex reads these files in order:

1. `Requirements/TOP_CODEX_RUNBOOK.md`
2. `Requirements/AGENT_ORCHESTRATION.md`
3. `Requirements/FINAL_GATE.md`
4. `Requirements/001/000_SCOPE_AND_ORDER.md`
5. `Requirements/001/001_ACCEPTANCE_CASES.md`
6. `Requirements/001/002_MICROSOFT_EDIT_ORACLE.md`
7. `Requirements/001/003_CURRENT_FAILURE_MAP.md`
8. `Requirements/001/004_INSTALL_ACCEPTANCE_HARNESS.md`
9. `Requirements/001/005_FREEZE_ACCEPTANCE_BASELINE.md`

The first Codex run is a setup run, not a Goal run. It creates and freezes the acceptance oracle before implementation.

After the setup run has created `Requirements/001/BASELINE_REF.txt` and stopped, start Goal Mode with the exact prompt in `Requirements/GOAL_PROMPT.md`.

## Non-negotiable rule

The Goal run may modify production code, normal implementation tests, and source documentation. It must not modify, delete, skip, weaken, or replace the frozen acceptance contract, acceptance tests, required-case manifest, verifier scripts, or Goal contract created by the setup run.

Passing a weakened test suite does not count as completion.
