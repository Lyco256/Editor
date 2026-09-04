# Top Codex — integration and initial-release acceptance

## 1. Wave 1 integration

The top Codex merges all successful Wave 1 branches into `devenv`.

After merges:

- regenerate `Cargo.lock`,
- resolve only cross-crate API mismatches required by the frozen contracts,
- run full workspace format check,
- run full workspace Clippy with warnings denied,
- run full workspace tests,
- run documentation mirror test.

Wave 2 does not start from a red `devenv`.

## 2. Wave 2 integration

All Wave 2 worktrees start at the same tested Wave 1 integration commit.

After Wave 2 branches complete, the top Codex merges them into `devenv`, regenerates the lockfile, wires root application actions/effects to each service, and executes the complete verification suite.

## 3. Required end-to-end scenarios

The integrated application is not complete until automated or deterministic headless integration tests cover these scenarios:

### Startup and shutdown

- start without arguments,
- start with a file,
- start with a directory,
- clean quit,
- dirty-buffer quit protection,
- terminal cleanup,
- restart and session restore.

### Editing

- create/edit/save/reopen,
- undo/redo,
- multiple cursors,
- auto pairs,
- Enter inside braces,
- paired deletion/overtype,
- indentation,
- file find/replace,
- format transaction undo.

### Workspace

- Explorer,
- Quick Open,
- multiple roots,
- project search,
- project replace preview and execution,
- `.gitignore`,
- file rename/move/delete.

### Encoding

- UTF-8,
- UTF-8 BOM,
- UTF-16 LE,
- UTF-16 BE,
- representative legacy encodings from `encoding_rs`,
- Shift_JIS-compatible Japanese fixture,
- CRLF preservation,
- mixed EOL detection.

### Syntax

Every required built-in language opens and produces syntax output from a representative fixture.

### LSP

Using the fake server:

- initialize,
- live diagnostics,
- completion,
- hover,
- signature help,
- definition/reference navigation,
- rename,
- code action,
- formatting,
- semantic tokens,
- inlay hints,
- cancellation,
- server crash and restart,
- Unicode position mapping.

### Workspace Trust

In an untrusted workspace, attempts to start:

- LSP,
- external formatter,
- Git

are blocked before process execution.

Trusting the workspace enables the same actions.

### Git

Using a temporary repository:

- detect changes,
- show diff,
- stage/unstage,
- hunk stage,
- commit,
- amend,
- branch create/switch,
- stash,
- conflict indication.

No normal test contacts a network remote.

### UI

Framebuffer snapshots cover:

- normal layout,
- 80x24,
- smaller collapse state,
- Explorer,
- tabs,
- split view,
- bottom Problems,
- command palette,
- diagnostics,
- completion,
- Git view,
- true-color and reduced-color semantic rendering.

### Large file

A large-file-mode integration test verifies:

- threshold activation,
- semantic subsystem suppression,
- text display/edit/save/search remains enabled,
- no LSP or syntax process starts for that document.

A stress fixture validates behavior at a size practical for automated tests. The architecture and runtime do not hard-reject a 1 GiB file solely based on file length.

## 4. Quality gates

The final `devenv` must pass:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
- source-document mirror verification
- repository verification script

There are no ignored failing tests used to hide defects.

There are no production `todo!()`, `unimplemented!()`, placeholder panics, dead temporary feature flags, or known data-loss defects.

## 5. Manual Windows Terminal smoke test

The top Codex performs a final Windows Terminal smoke test when the execution environment permits it and records the result in `docs/testing/final-smoke.md`.

The smoke checklist covers:

- startup,
- keyboard editing,
- mouse selection,
- clipboard,
- resize,
- tabs/splits,
- Explorer,
- command palette,
- diagnostics visual fallback,
- Git view,
- exit and terminal restoration.

If the environment cannot provide an interactive Windows Terminal, the limitation is recorded and all headless/backend tests remain mandatory.

## 6. Performance verification

Record release-build measurements in `docs/testing/performance.md` for:

- release executable and mandatory asset size,
- empty-workspace resident memory,
- idle CPU observation,
- normal editing of a 10 MiB file,
- repeated open/close memory behavior.

A regression that violates the master performance budgets blocks release unless explicitly approved by the user.

## 7. Documentation completion

All production Rust source files have mirrored docs.

Additionally, `docs/architecture/` contains:

- architecture overview,
- event/effect flow,
- crate dependency diagram,
- text coordinate systems,
- rendering pipeline,
- LSP lifecycle,
- Workspace Trust boundary,
- Git process boundary,
- recovery/session format.

`docs/testing/` contains:

- test strategy,
- fixture strategy,
- final integration results,
- performance results,
- final smoke result.

## 8. Merge to main

Only after every release gate above passes does the top Codex merge `devenv` into `main`.

Completion requires:

- `main` contains the same tested product state as the passing `devenv`,
- `main` passes the full verification suite after merge,
- working trees are clean,
- no required feature branch remains unmerged,
- no release-blocking issue remains unresolved.

A visually plausible application with failing tests, incomplete docs, disabled trust checks, placeholder implementations, unmerged feature work, or a `main` state different from tested `devenv` is not complete.
