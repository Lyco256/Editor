# Subagent requirement — Git backend

## Branch

`feat/git-backend`

## Writable ownership

- `crates/vcs-git/**`
- `docs/crates/vcs-git/**`
- `tests/fixtures/vcs-git/**`

Do not edit root application files, shared types, other crates, or `Cargo.lock`.

## Scope

Implement Git integration by invoking the installed `git` executable.

Do not use libgit2.

## Required operations

- Git executable discovery,
- repository root detection,
- status,
- current branch / detached HEAD state,
- working-tree diff,
- index diff,
- stage file,
- stage hunk,
- unstage file,
- unstage hunk where safely expressible,
- discard planning,
- commit,
- amend,
- branch list,
- branch create,
- branch switch,
- branch delete,
- fetch,
- pull,
- push,
- stash list/create/apply/pop,
- basic log/history,
- conflict-file detection.

Destructive operations return a plan/confirmation requirement to the caller before execution where user confirmation is required.

## Process behavior

- No shell string concatenation.
- Invoke `git` with structured argument arrays.
- Preserve normal Git credential/helper behavior.
- Capture stdout/stderr separately.
- Support cancellation for long-running operations.
- Parse stable machine-readable Git output formats where available.
- Never run an external process unless the root trust/policy layer authorizes it.

## Tests

Create temporary Git repositories using the real installed `git`.

Tests configure local repository user name/email so they do not depend on machine-global identity.

Required tests include:

- repository discovery,
- clean/modified/untracked/staged status,
- diff parsing,
- stage/unstage,
- hunk stage,
- commit/amend,
- branches,
- stash,
- log,
- merge-conflict detection,
- paths containing spaces and Unicode,
- command failure and stderr propagation.

Network operations are unit-tested through argument construction/fake process boundaries; they do not contact a real remote during normal tests.

## Acceptance criteria

- The backend has no terminal UI dependency.
- All Git commands use structured arguments.
- Local repository integration tests pass.
- No normal test requires network access.
- `cargo test -p vcs-git` and Clippy pass.
- All owned source files have mirrored documentation.
