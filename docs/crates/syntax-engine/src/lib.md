# Syntax engine boundary

Role: resolves and parses syntax for a single document, then derives editor-facing structural data from the resulting Tree-sitter tree.

Public surface:
- `ParseTicket` tags parse work with a document/version pair so stale results can be rejected.
- `SyntaxLanguage` identifies the supported built-in language set and handles path-based detection.
- `SyntaxEngine` stores per-document parser state, applies full-document replacements incrementally when possible, and exposes selection and pair queries.
- `SyntaxSnapshot`, `SyntaxSpan`, `SyntaxSymbol`, and `SyntaxPair` capture parse results in theme-independent editor types.

Important invariants:
- Large-file mode suppresses parsing instead of attempting a syntax tree update.
- Parse results are always versioned, and stale updates are rejected before state is overwritten.
- Incremental edits reuse the previous tree only when the language and document identity still match.
- Character-based editor ranges are converted from Tree-sitter byte ranges inside the crate so callers do not have to mix coordinate systems.

Data flow:
1. Callers open or update a document with a versioned descriptor, text, and optional language override.
2. The engine resolves the language from the override or path.
3. The engine parses with the appropriate bundled Tree-sitter grammar and reuses the previous tree when it can.
4. The engine derives highlights, folds, symbols, error regions, and pair metadata from the tree and source text.
5. Position-sensitive queries such as structural selection and matching pairs read the stored document snapshot.

Concurrency and cancellation:
- The crate is synchronous and single-threaded by design.
- Cancellation is accepted through a small probe trait and is checked before parse work begins and during parse progress callbacks.

Error behavior:
- Unknown languages are reported with `SyntaxStatus::MissingLanguage`.
- Cancelled work returns `SyntaxStatus::Cancelled` without replacing the stored snapshot.
- Stale versions return `SyntaxStatus::StaleVersion` and leave existing state intact.
- Large-file mode returns `SyntaxStatus::LargeFileSuppressed` and stores a suppressed snapshot.

Dependencies:
- `editor-core` for the versioned document descriptor.
- `editor-types` for document ids, text ranges, cursor offsets, and style roles.
- `tree-sitter` and the bundled grammar crates for all required languages.

Tests:
- Parser smoke coverage for all 16 languages.
- Highlight, fold, symbol, and pair coverage on the full fixtures.
- Incomplete-source parsing.
- Incremental edit reuse.
- Stale version rejection.
- Cancellation handling.
- Mixed Unicode source handling.
- Large-file suppression.
