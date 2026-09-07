# Subagent requirement — Git UI

## Branch

`feat/ui-git`

## Wave

Wave 2. Start only from the tested Wave 1 `devenv` commit.

## Writable ownership

- `crates/app-ui/src/git/**`
- corresponding `docs/crates/app-ui/src/git/**`
- `tests/fixtures/app-ui/git/**`
- `tests/snapshots/ui-git/**`

Do not edit shared UI shell files, root application files, other UI feature directories, or `Cargo.lock`.

## Scope

Implement a terminal-native Source Control UI inspired by VS Code and focused on fast keyboard use.

Required views/actions:

- repository summary,
- branch display,
- changed files grouped by staged/unstaged/untracked/conflict state,
- diff view,
- working-tree/index toggle,
- file stage/unstage,
- hunk stage/unstage,
- discard confirmation,
- commit message entry,
- commit,
- amend,
- branch list/create/switch/delete,
- fetch,
- pull,
- push,
- stash list/create/apply/pop,
- basic commit log/history,
- conflict-file indication.

Git change markers are supplied to the editor gutter and overview ruler through shared marker models.

## UX rules

- Destructive actions require confirmation.
- Long Git operations expose busy/progress state without freezing the UI.
- Git stderr/failure is surfaced in Output and relevant inline status.
- Credential prompts remain controlled by normal Git behavior; the UI does not store credentials.
- Untrusted workspaces show Git as disabled by policy.

## Tests

Framebuffer/state tests cover:

- clean repo,
- mixed change states,
- staged/unstaged transitions,
- diff,
- hunk selection,
- commit form,
- branch chooser,
- stash list,
- conflict state,
- command failure,
- untrusted disabled state,
- narrow layout.

## Acceptance criteria

- UI never constructs shell command strings.
- UI calls typed Git actions only.
- `cargo test -p app-ui` passes for Git UI cases.
- Clippy passes.
- All owned source files have mirrored documentation.
