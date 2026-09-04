# Editor agent contract

Read `Requirements/00_MASTER_REQUIREMENTS.md`, `Requirements/01_AGENT_AND_GIT_RULES.md`,
`Requirements/02_ARCHITECTURE_AND_PROJECT_LAYOUT.md`, and the requirement assigned to your
branch before editing.

- `main` is stable; `devenv` is integration; feature work uses `feat/<scope>` branches.
- Only the top agent edits root orchestration, workspace membership, `Cargo.toml`, `Cargo.lock`,
  `src/app/**`, `crates/editor-types/**`, or `crates/app-ui/src/lib.rs`.
- Feature agents write only paths granted by their requirement file and never merge or rebase.
- Do not commit generated output or feature-branch `Cargo.lock` changes.
- Every production `.rs` file must have a path-identical Markdown mirror below `docs/`.
- Expected user/environment errors are typed; production `todo!`, `unimplemented!`, temporary
  panics, ignored failing tests, and silent data-loss paths are forbidden.
- Before reporting completion, run formatting, Clippy with warnings denied, owned tests, the doc
  mirror test, and confirm a clean worktree.
- External processes are authorized only by the root Workspace Trust policy and are invoked with
  structured executable/argument values, never shell concatenation.

