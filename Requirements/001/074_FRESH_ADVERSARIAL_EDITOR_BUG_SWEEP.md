# 074 — fresh adversarial editor bug sweep

Owner: fresh read-only reviewer agent

Writable paths:
- no production writes
- report only: `docs/testing/requirements-001-review-adversarial.md`

## Objective

Try to break the completed editor using code inspection and new automated probes focused on edge cases not directly mirrored by implementation tests.

## Required implementation

Inspect/probe:
- empty file;
- one-line file;
- trailing newline and no trailing newline;
- CRLF;
- combining text;
- CJK;
- tabs;
- very long line;
- empty lines;
- tiny terminal;
- repeated resize;
- split with same/different docs;
- dirty Preview;
- menu/palette focus vs typing;
- drag while scrolled;
- cursor at EOF;
- deletion at boundaries;
- Undo after multi-step editor commands.

Do not edit production code. Temporary probes may be run from temporary files and discarded.

Every reproducible defect violating Requirements 001 is a blocker.

End `BLOCKERS: N`.

## Required tests

No code changes.

## Done only when

independent adversarial pass finds no remaining reproducible requirement defect: `BLOCKERS: 0`.
