# Requirements 002 — Goal prompt

Start Goal Mode only after `Requirements/002/BASELINE_REF.txt` is valid and
`tools/verify-002` passes in pre-goal mode. Continue the blocked 001 Goal on the
verified `devenv` state, preserving all frozen 002 oracle files and the repaired
K004/K005/K006 acceptance adapter. Implement only the remaining product work
described by `Requirements/GOAL_PROMPT.md` and the active Requirements files.

Goal Mode may not edit canonical vectors, the independent reference checker,
the repaired acceptance adapter, verifier semantics, baseline reference, or the
written 002 oracle. Completion requires the final 002 gate, including both
independent final reviews, all 78 acceptance cases, workspace quality checks,
documentation mirrors, and a clean worktree.
