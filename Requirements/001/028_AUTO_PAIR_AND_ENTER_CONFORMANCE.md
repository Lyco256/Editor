# 028 — auto pair and Enter conformance

Owner: CORE worker

Writable paths:
- existing editor-core smart-edit modules
- matching docs/tests

## Objective

Re-verify and tighten pair insertion/deletion/Enter behavior so ordinary editing does not regress while navigation is rebuilt.

## Required implementation

For configured pairs:
1. opening delimiter inserts closing delimiter;
2. typing an already auto-inserted closer overtypes/moves through it rather than duplicating;
3. Backspace inside an untouched empty pair removes both;
4. Enter inside `{}` expands to opening line, indented blank caret line, closing line;
5. indentation uses effective settings;
6. quotes do not blindly pair when language configuration marks context where they should not;
7. all pair edits are one undo transaction per user action.

## Required tests

- dedicated `{}`, `[]`, `()`, quotes fixtures
- CRLF and LF
- tabs/spaces
- undo/redo
- multi-cursor pair insertion

## Done only when

existing smart-edit behavior remains correct under the new navigation/deletion architecture.
