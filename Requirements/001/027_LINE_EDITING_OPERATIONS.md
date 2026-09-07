# 027 — line editing operations

Owner: CORE worker

Writable paths:
- `crates/editor-core/src/navigation.rs`
- matching docs/tests

## Objective

Implement mature-editor line commands as transaction-safe core operations.

## Required implementation

Implement exactly:
- move selected/current line block up;
- move block down;
- copy block up;
- copy block down;
- insert line below;
- insert line above;
- select current logical line;
- select all document.

Normalize overlapping multi-cursor line blocks before mutating so the same line is not moved/copied twice.

Preserve line endings.

A move at top/bottom where movement is impossible is a no-op without corrupting selection.

## Required tests

- `K019`–`K022`
- top/bottom boundary
- multi-line selection
- CRLF
- multiple cursors
- undo/redo

## Done only when

all listed line commands are exposed by editor-core and preserve text/selection invariants.
