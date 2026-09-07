# Subagent requirement — Tree-sitter syntax engine

## Branch

`feat/syntax-engine`

## Writable ownership

- `crates/syntax-engine/**`
- `docs/crates/syntax-engine/**`
- syntax queries/assets owned by syntax under `assets/tree-sitter/**`
- `tests/fixtures/syntax-engine/**`

Do not edit root application files, shared types, other crates, or `Cargo.lock`.

## Scope

Implement incremental Tree-sitter parsing and structural syntax services for the required initial languages.

Required languages:

- Rust
- C
- C++
- C#
- Java
- Go
- Python
- JavaScript
- TypeScript
- HTML
- CSS
- JSON
- TOML
- YAML
- Markdown
- shell script

## Required services

- language detection from path/extension with explicit override,
- parser lifecycle,
- incremental reparse after text edits,
- syntax highlight capture model,
- structural folding ranges,
- matching-pair assistance,
- structural selection ranges,
- syntax/document symbols where grammar queries support them,
- syntax error regions,
- cancellation/staleness/version protection.

Syntax output uses semantic highlight roles from shared types and does not contain terminal colors.

## Large-file behavior

When the document reports large-file mode, the syntax engine does not start parsing the document.

## Correctness

A parse result is tagged with the source document version.

A stale parse result is rejected by the integration layer and never overwrites newer syntax state.

Malformed/incomplete code still produces partial syntax output where Tree-sitter permits it.

## Tests

Every supported language has:

- grammar initialization smoke test,
- highlight fixture,
- incomplete-code fixture,
- fold or structural-range fixture where language syntax provides one.

Additional tests cover:

- incremental edit updates,
- stale version rejection data,
- parser errors,
- cancellation,
- mixed Unicode source.

## Acceptance criteria

- All required languages initialize without runtime downloads.
- Parser work runs outside the UI update path.
- Syntax roles are theme-independent.
- `cargo test -p syntax-engine` and Clippy pass.
- All owned source files have mirrored documentation.
