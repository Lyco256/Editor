# Subagent requirement — VS Code static compatibility

## Branch

`feat/vscode-static-compat`

## Writable ownership

- `crates/vscode-compat/**`
- `docs/crates/vscode-compat/**`
- `tests/fixtures/vscode-compat/**`

Do not edit root application files, shared types, other crates, or `Cargo.lock`.

## Scope

Implement static-data interoperability with useful VS Code formats without executing VS Code extensions.

## Supported inputs

- `.vscode/settings.json` JSONC,
- user/workspace keybinding data that maps to Editor command IDs,
- VS Code color-theme JSON,
- VS Code snippet JSON,
- VS Code language configuration JSON,
- static supported contribution points from a local extension directory,
- static supported contribution points extracted from a local VSIX archive.

## Supported static extension contributions

Initial supported contribution points:

- themes,
- snippets,
- languages,
- language configuration references.

Tree-sitter remains the Editor syntax engine. VS Code TextMate grammar contributions are detected but not activated as the primary parser/highlighter in the initial release.

## Unsupported extension behavior

The initial release does not execute:

- extension JavaScript/TypeScript,
- Node.js Extension Host code,
- WebViews,
- custom GUI panels,
- Electron APIs,
- arbitrary activation events,
- extension-provided commands requiring executable extension code.

Unsupported contribution points produce a structured compatibility warning available to Output.

## Security

Loading static JSON/JSONC/VSIX content does not execute code.

Archive extraction rejects path traversal and writes only inside the selected cache/install directory.

Malformed extension metadata does not crash the application.

## Tests

Fixtures cover:

- JSONC settings,
- theme mapping,
- snippets with tab stops/placeholders,
- language config pairs/comments/indent rules,
- local extension directory,
- local VSIX,
- unsupported contribution warning,
- malformed manifest,
- zip path traversal rejection.

## Acceptance criteria

- Static supported data is converted into config/theme/snippet/language models.
- No extension executable code is run.
- VSIX extraction is path-safe.
- `cargo test -p vscode-compat` and Clippy pass.
- All owned source files have mirrored documentation.
