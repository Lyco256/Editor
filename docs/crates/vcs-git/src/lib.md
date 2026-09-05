# Git adapter boundary

Role: provides the Wave 1 Git process adapter for Editor. This crate discovers repositories,
reads status and branch state, parses working-tree and index diffs, stages and unstages files and
hunks, plans and executes discard operations, runs commit/amend and branch workflows, and exposes
fetch/pull/push/stash/log/conflict helpers through structured `git` process invocation.

## Public surface

- `DiffTarget` distinguishes working-tree versus index diff/discard operations.
- `CancellationToken` gives the adapter a cancellation hook without depending on UI state.
- `GitClient` owns the configured `git` executable and runs every command.
- `GitInvocation` records a command and its working directory for testing and diagnostics.
- `GitCommandOutput` preserves status, stdout, and stderr separately.
- `GitRepositoryStatus`, `GitBranchState`, `GitStatusEntry`, `GitDiffFile`, `GitDiffHunk`, and
  related structs carry parsed repository state.
- `GitCommitRequest`, `GitFetchRequest`, `GitPullRequest`, `GitPushRequest`, and
  `GitStashCreateRequest` describe command arguments without shell concatenation.
- `GitDiscardPlan` records confirmation state and the exact hunk/file scope for destructive work.

## Invariants

- All process calls use structured argument vectors and a direct `git` executable.
- The adapter never embeds libgit2 or reaches into terminal UI state.
- Diff parsing uses stable Git output formats and preserves file paths containing spaces or Unicode.
- Hunk staging and unstaging use generated patch text, `git apply`, and zero-context diffs.
- Discard execution requires the caller to confirm plans that would lose tracked or untracked work.

## Data flow

1. The caller asks for repository discovery or an operation against a workspace path.
2. `GitClient` resolves the repository root when needed and builds a `GitInvocation`.
3. `run_command` launches `git`, captures stdout and stderr separately, and respects cancellation.
4. Parsers convert porcelain status, name-status metadata, unified diffs, branch listings, stash
   listings, and log output into typed models.
5. Destructive operations either return a `GitDiscardPlan` or require an explicit confirmation flag
   before execution.

## Error behavior

- Missing executables, cancellation, parse failures, repository lookup failures, confirmation
  failures, and command failures are all surfaced as `GitError`.
- Command failures preserve the command string, exit code, and stderr text.
- Empty repositories are handled without treating the missing `HEAD` revision as a fatal status error.

## Tests

- Repository detection and status parsing.
- Working-tree and index diff parsing, including Unicode and space-containing paths.
- File staging, hunk staging, file unstaging, and hunk unstaging.
- Commit and amend flows.
- Branch create/switch/delete, stash create/list/apply/pop, and log parsing.
- Conflict detection and command-failure stderr propagation.
- Structured argument construction for fetch, pull, and push.
