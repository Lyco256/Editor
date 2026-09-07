# 005 — authorized oracle migration

Owner: top Codex. Normal setup mode only.

1. Confirm branch `devenv` and clean worktree.
2. Record current HEAD in `docs/testing/requirements-002-pre-migration.md`.
3. Create local safety branch `backup/requirements-001-blocked-<shortsha>`.
4. Do not reset/revert production commits.
5. Run the supplied Python vector checker and require exit 0.
6. Locate actual K004/K005/K006 acceptance functions by case ID; record their paths and old fixtures/expectations.
7. Modify only those three acceptance adapters: remove invalid local fixture/expected constants; load matching canonical vector variants; execute the real production preferred-column operation; assert every checkpoint.
8. Do not change production code in this setup run.
9. Re-run all other 75 original acceptance cases and require they remain passing.
10. Run workspace tests, format, Clippy, docs mirror.
11. If K004–K006 still fail against canonical vectors, report a real product mismatch; do not alter vectors.
