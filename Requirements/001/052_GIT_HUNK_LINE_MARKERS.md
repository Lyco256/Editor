# 052 — Git hunk line markers

Owner: top Codex

Writable paths:
- `src/app/scene.rs`
- existing Git projection glue
- matching docs/tests

## Objective

Build gutter/overview Git markers from actual diff hunks instead of marking a changed file's entire document.

## Required implementation

1. Delete ordinary changed-file marker `0..buffer.len_chars()`.
2. Use existing Git diff model/hunks.
3. Advance old/new line counters from hunk metadata and parsed diff line kinds.
4. Added new lines -> GitAdded.
5. replacement affected new-file lines -> GitModified.
6. pure deletions -> GitDeleted anchored at nearest surviving new-file line/gutter position.
7. untracked text file may mark its existing lines GitAdded.
8. conflict status is shown as conflict state; do not paint every source line Error solely because file is conflicted.
9. Refresh after diff/status refresh, save, stage, unstage, discard, active-document change.
10. Diff work remains outside UI mutation path.

## Required tests

- `P006_GIT_HUNK_LINES`
- `P007_GIT_ADD_DELETE`
- 100-line one-line modification
- addition/deletion/replacement fixtures

## Done only when

Git visual markers identify actual changed locations, never a whole document for an ordinary hunk.
