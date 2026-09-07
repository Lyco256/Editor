# 005 — freeze the acceptance baseline

Owner: top Codex.
Mode: setup run only.

## Frozen paths

The first baseline commit includes and freezes:

- `Requirements/README.md`
- `Requirements/TOP_CODEX_RUNBOOK.md`
- `Requirements/AGENT_ORCHESTRATION.md`
- `Requirements/GOAL_PROMPT.md`
- `Requirements/FINAL_GATE.md`
- every `.md` file under `Requirements/001/` except `BASELINE_REF.txt`
- `tests/requirements_001/**`
- `tools/requirements-001/required-cases.txt`
- `tools/verify-001.ps1`
- `tools/verify-001.sh`

## Commit sequence

1. Ensure the current branch is `devenv`.
2. Ensure production fixes have not been made by the setup run.
3. Commit the complete frozen paths plus acceptance harness as:
   `test(001): freeze Editor interaction acceptance oracle`
4. Record the full 40-hex SHA of that commit.
5. Create `Requirements/001/BASELINE_REF.txt` containing only that SHA and newline.
6. Commit only `BASELINE_REF.txt` as:
   `chore(001): record frozen acceptance baseline`
7. Run the verifier's baseline-integrity check and prove it sees an empty diff for all frozen paths.
8. Stop the setup run.

## Goal-run prohibition

During Goal Mode, neither the top Codex nor any subagent may modify frozen paths.

If an implementation appears impossible under a frozen test, the implementation is considered wrong until an independent read-only reviewer proves the test contradicts this written oracle.

The Goal agent may not self-authorize an oracle change.

Any needed oracle correction requires a new user-approved requirement cycle, not a Goal-run edit.
