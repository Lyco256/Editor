# 076 — completion and integration decision

Owner: top Codex

Writable paths:
- Git integration state
- `docs/testing/requirements-001-final.md`

## Objective

Decide completion mechanically, not by confidence or appearance.

## Required implementation

1. Run requirement 075.
2. Run `git status --short`; require clean.
3. Record final `devenv` SHA.
4. Record frozen baseline SHA.
5. Record zero exit status and all reviewer zero-blocker lines.
6. Confirm every lane branch required by orchestration is merged into `devenv`.
7. Do not merge to `main` unless the repository's existing branch policy requires it for this cycle and all gates remain green after that merge.
8. If any gate is non-zero, return to the owning task; do not write a completion report saying “mostly complete”.

## Required tests

No additional behavioral test; this task consumes all prior gates.

## Done only when

all objective gate evidence is green. Otherwise the Goal remains active.
