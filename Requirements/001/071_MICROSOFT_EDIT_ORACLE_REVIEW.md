# 071 — Microsoft Edit oracle review

Owner: fresh read-only reviewer agent

Writable paths:
- no production writes
- report only: `docs/testing/requirements-001-review-edit.md`

## Objective

Find any remaining mismatch between Editor and the Microsoft Edit behaviors in requirement 002.

## Required implementation

Review current production code and tests for resize, rendered-layout hit testing, persistent textarea state, mouse selection, preferred column, Left/Right, Home/End, Page, Ctrl scroll, Delete/Backspace, indentation, cursor reveal, and menubar.

Do not trust test names. Trace the actual route.

Report each blocker with source file/function, expected oracle behavior, actual behavior, and a reproducible automated test suggestion.

End report with exactly `BLOCKERS: N`.

## Required tests

No code changes. A zero-blocker report is required for final completion.

## Done only when

the reviewer has traced all oracle categories and either found concrete blockers or states `BLOCKERS: 0`.
