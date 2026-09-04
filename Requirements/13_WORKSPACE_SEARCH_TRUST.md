# Subagent requirement — workspace, file operations, search, and trust

## Branch

`feat/workspace-search-trust`

## Writable ownership

- `crates/workspace-core/**`
- `docs/crates/workspace-core/**`
- `tests/fixtures/workspace-core/**`

Do not edit root application files, shared types, other crates, or `Cargo.lock`.

## Scope

Implement the workspace model and non-UI services for:

- one or more workspace roots,
- canonical path handling,
- text document byte loading through config-core encoding detection,
- atomic text document save through config-core encoding/EOL conversion,
- external file-change watching,
- Explorer tree model,
- file discovery,
- create/rename/move/delete,
- Quick Open candidate indexing,
- project search,
- project replace planning,
- `.gitignore` handling,
- configured exclude patterns,
- recent-workspace model,
- workspace trust state.

## Search

Prefer installed `rg` for project search.

When `rg` is absent, use the internal Rust fallback.

Both backends support the same required user options:

- literal search,
- regex search,
- case-sensitive,
- case-insensitive,
- whole-word,
- file include filter,
- file exclude filter.

Search streams results and supports cancellation.

Search never blocks the UI thread.

Replacement is planned as an explicit set of file edits before writes begin. A failure during multi-file replacement reports which files were modified and which failed.

## Document I/O and external changes

Opening a text document reads bytes in the workspace service and delegates encoding/EOL interpretation to config-core. Saving receives an explicit text snapshot plus encoding/EOL metadata and performs temp-file + flush + replace/rename semantics so a failed write does not truncate the last valid file.

The workspace service watches open files and Explorer roots using a native filesystem watcher abstraction. If an open buffer is clean and the file changes externally, the service emits a reload event. If an open buffer is dirty, it emits a conflict event and never overwrites either version automatically.

## Filesystem safety

- Paths are canonicalized where required for trust/security decisions.
- Symlink loops do not recurse indefinitely.
- Explorer traversal is lazy.
- Binary files are not treated as normal text search/replace targets unless the search mode explicitly permits it later.
- Delete and overwrite operations expose enough metadata for the UI to request confirmation.
- Case-only renames work correctly on Windows.

## Workspace Trust

Trust state is stored by canonical workspace identity.

The model exposes trusted/untrusted state to root orchestration.

An untrusted workspace blocks external process effects at the root policy layer. This crate never bypasses the policy.

## Tests

Use temporary directory trees.

Required tests include:

- multi-root identity,
- `.gitignore`,
- excludes,
- symlink loop protection,
- Quick Open indexing,
- text document open/decode integration,
- atomic save failure safety,
- external file change for clean and dirty buffers,
- cancellation,
- `rg` result parsing with a fake process/result fixture,
- fallback search equivalence for required options,
- rename/move/delete planning,
- Windows case-insensitive path semantics behind platform helpers,
- trust persistence key stability.

## Acceptance criteria

- Tree enumeration is lazy.
- Search is cancellable and streaming.
- No UI types are required by the crate.
- `cargo test -p workspace-core` and Clippy pass.
- All owned source files have mirrored documentation.
