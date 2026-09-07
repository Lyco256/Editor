# 033 — double and triple click selection

Owner: POINTER worker

Writable paths:
- `crates/app-ui/src/editor/pointer.rs`
- matching docs/tests

## Objective

Implement deterministic click-count selection behavior matching Edit's double/triple-click baseline.

## Required implementation

1. The normalized input supplies click count 1/2/3.
2. count 2 selects word at pointer using supplied language word policy/fallback.
3. count 3 selects entire logical line, including line ending when present.
4. a drag is not interpreted as another click.
5. clicking another cell starts a new click sequence in the lower input layer.
6. Pointer UI does not implement wall-clock timing itself.

## Required tests

- `M008_DOUBLE_CLICK_WORD`
- `M009_TRIPLE_CLICK_LINE`
- punctuation/whitespace word tests
- final line without newline

## Done only when

word and line selection are explicit actions with exact ranges and no UI timing guesses.
