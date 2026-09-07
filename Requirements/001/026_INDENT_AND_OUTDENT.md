# 026 — indent and outdent

Owner: CORE worker

Writable paths:
- `crates/editor-core/src/navigation.rs`
- matching docs/tests

## Objective

Implement Tab/ShiftTab using effective indentation settings rather than literal text insertion.

## Required implementation

1. Operation accepts tab width and insert-spaces mode.
2. Caret without multiline selection: insert indentation to next indentation stop.
3. Multi-line/non-empty line selection: indent every affected logical line once.
4. Shift+Tab removes up to one indentation unit from each affected line without deleting non-whitespace.
5. Preserve/transform selections deterministically.
6. One command -> one transaction.
7. Tabs/wide display columns use display-column helpers.

## Required tests

- `K017_TAB_INDENT`
- `K018_SHIFT_TAB_OUTDENT`
- tab widths 2/4/8
- spaces vs tabs
- multiline selection
- undo/redo

## Done only when

indentation behavior follows effective document settings and never hard-codes width 4.
