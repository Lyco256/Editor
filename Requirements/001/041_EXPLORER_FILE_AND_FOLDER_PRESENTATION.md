# 041 — Explorer file and folder presentation

Owner: WORKBENCH worker

Writable paths:
- `crates/app-ui/src/shell/explorer.rs`
- matching docs/tests

## Objective

Make file/folder identity unambiguous without font-dependent icons.

## Required implementation

1. Explorer row model has explicit File/Directory kind.
2. Directory name ends `/`.
3. Collapsed directory prefix `[+] `.
4. Expanded directory prefix `[-] `.
5. File prefix is four spaces.
6. Optional tree guides use ASCII/Box Drawing only.
7. Directory and file use distinct semantic foreground roles.
8. Selected row has distinct foreground/background.
9. Exact row rectangles and entry IDs are written to layout snapshot.
10. Single-click file emits Preview request; double-click/Enter permanent Open; Space Preview.
11. Directory marker/Enter/double-click toggles expansion.
12. Explorer wheel scroll keeps selected row visible.

## Required tests

- `U004_FILE_FOLDER_DISTINCT`
- color-reduced snapshot
- file/directory interaction
- resize/scroll row hit identity

## Done only when

file/folder difference is visible without color and pointer behavior uses exact row identity.
