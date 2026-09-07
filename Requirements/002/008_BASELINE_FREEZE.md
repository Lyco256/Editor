# 008 — freeze reconciled 002 baseline

After 004–007 pass, commit the authorized oracle migration as:
`test(002): reconcile preferred-column acceptance oracle`

Frozen paths are: `Requirements/002_README.md`, `Requirements/002_GOAL_PROMPT.md`, `Requirements/002_FINAL_GATE.md`, every file under `Requirements/002/` except `BASELINE_REF.txt`, the supplied reference checker, repaired K004–K006 acceptance adapter file(s), and `tools/verify-002.ps1/.sh`.

Record that commit's full SHA in `Requirements/002/BASELINE_REF.txt`, then commit only that file as:
`chore(002): record reconciled oracle baseline`

The active Goal integrity check now uses the 002 baseline. The old 001 baseline remains historical evidence and is explicitly superseded for the authorized K004–K006 adapter migration.
