# 024 — word navigation and word deletion

Owner: CORE worker

Writable paths:
- `crates/editor-core/src/navigation.rs`
- matching docs/tests

## Objective

Implement word movement/deletion with caller-supplied word rules and deterministic fallback.

## Required implementation

1. Expose previous/next word-boundary operations.
2. Ctrl+Left/Right use them.
3. Shift variants extend.
4. Ctrl+Backspace deletes previous word span.
5. Ctrl+Delete deletes next word span.
6. Caller can provide language word-pattern/rules; fallback treats identifier letters/digits/underscore, whitespace, and punctuation as distinct classes.
7. Operations are Unicode safe and one transaction per deletion command.

## Required tests

- `K013_WORD_LEFT_RIGHT`
- `K014_WORD_DELETE`
- identifiers, punctuation, whitespace, Unicode letters
- start/end boundaries

## Done only when

word behavior is implemented in core and root does not reimplement offsets manually.
