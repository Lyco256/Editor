# Subagent requirement — config, encoding, and recovery

## Branch

`feat/config-encoding-recovery`

## Writable ownership

- `crates/config-core/**`
- `docs/crates/config-core/**`
- `tests/fixtures/config-core/**`

Do not edit root application files, shared types, other crates, or `Cargo.lock`.

## Scope

Implement configuration merge rules, `.editorconfig` loading, encoding detection/conversion, editor settings, keybinding data, theme data model, session persistence, and crash/unsaved-buffer recovery storage.

## Configuration precedence

Lowest to highest precedence:

1. built-in defaults,
2. user settings,
3. workspace settings,
4. folder-specific settings for the active workspace root,
5. in-memory session overrides.

Unknown keys are preserved when the file is rewritten only if rewriting is explicitly required. Normal loading never destroys unknown settings.

JSONC parsing supports comments and trailing commas for VS Code-compatible settings.

`.editorconfig` is loaded from the active document directory upward according to EditorConfig precedence. At minimum, `indent_style`, `indent_size`, `tab_width`, `end_of_line`, `charset`, `trim_trailing_whitespace`, and `insert_final_newline` are mapped into document-effective settings. Workspace/user settings remain available, but an applicable `.editorconfig` value governs the corresponding document formatting behavior.

## Required settings model

Include settings for:

- line numbers,
- tab size,
- insert spaces,
- word wrap,
- auto-closing pairs,
- format on save,
- format on paste,
- theme,
- search excludes,
- files excludes,
- large-file threshold,
- encoding fallback,
- line ending preference,
- known language-server command overrides,
- external formatter command overrides,
- keybindings.

## Encoding

Use:

- BOM detection first,
- UTF-8 validation second,
- `chardetng` for heuristic legacy detection,
- configured fallback last.

Support the canonical encoding set and aliases exposed by `encoding_rs`, plus explicit UTF-16 LE/BE read/write.

Preserve detected encoding on save unless explicitly changed.

Preserve LF/CRLF by default and detect mixed line endings.

Round-trip tests include Japanese Shift_JIS-compatible text, Windows legacy code pages covered by `encoding_rs`, UTF-8 BOM, UTF-16 LE/BE, and invalid byte sequences with explicit error/replacement policy.

## Recovery

Persist session and unsaved-buffer recovery in the OS application-data directory.

Recovery writes use temp-file + flush + atomic rename/replacement semantics where supported.

Recovery records include enough metadata to restore:

- workspace roots,
- tabs,
- split layout,
- active editor,
- cursor/selection state,
- unsaved text,
- dirty state,
- original path if any,
- original encoding and line ending.

Recovery never writes unsaved text into the workspace path.

## Tests

Required tests include:

- settings precedence,
- `.editorconfig` directory traversal and precedence,
- `.editorconfig` mapping for all required properties,
- JSONC comments/trailing commas,
- malformed settings fallback with surfaced error,
- encoding detection order,
- encoding round trips,
- mixed EOL detection,
- atomic recovery replacement behavior using temp directories,
- recovery from truncated/corrupted newest record using last valid record or safe failure,
- session serialization compatibility within the current format version.

## Acceptance criteria

- Configuration loading does not execute processes.
- Encoding operations are deterministic.
- Unsaved contents survive simulated restart.
- Corrupt recovery data does not prevent the editor from starting.
- `cargo test -p config-core` and Clippy pass.
- All owned source files have mirrored documentation.
