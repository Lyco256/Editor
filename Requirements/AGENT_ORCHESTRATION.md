# Agent and worktree orchestration

## Principle

Use parallel agents only for write-disjoint ownership. A worker receives one lane, one worktree, one feature branch, and the small requirement files for that lane in numeric order.

Do not ask a worker to read all of `Requirements/001/` at once. Give it only:

- `000_SCOPE_AND_ORDER.md`,
- `001_ACCEPTANCE_CASES.md`,
- `002_MICROSOFT_EDIT_ORACLE.md` when relevant,
- `007_BRANCH_AND_OWNERSHIP.md`,
- the single task file it is currently implementing.

After a task passes its local gate, tell the same lane worker to read the next task file.

## Top-owned sequential work

The top Codex alone performs:

- 004 acceptance harness installation;
- 005 acceptance freeze;
- 006 mechanical decomposition;
- 010 runtime resize;
- 011 authoritative scene/layout snapshot;
- 012 pointer target routing contract;
- 013 persistent pane state;
- 014 authoritative document state;
- 015 no-op input guard;
- 050 LSP overlay projection integration;
- 051 exact project-search projection;
- 052 exact Git hunk projection;
- 053 secondary-pane isolation;
- 070 final feature integration;
- 075 final verifier;
- 076 completion/merge decision.

These tasks touch central orchestration and are not delegated to write agents.

## Parallel implementation lanes

After requirement 015 is green, create these worktrees from the exact same `devenv` foundation commit.

### Lane CORE

Branch: `feat/001-editor-core`

Writable:

- `crates/editor-core/**`
- matching `docs/crates/editor-core/**`
- non-frozen editor-core tests outside `tests/requirements_001/**`

Tasks, strictly in order:

- 020
- 021
- 022
- 023
- 024
- 025
- 026
- 027
- 028
- 029

The worker makes one commit per task.

### Lane TERMINAL

Branch: `feat/001-terminal`

Writable:

- `crates/terminal-backend/**`
- matching docs
- non-frozen terminal-backend tests

Tasks:

- 030

One commit.

### Lane POINTER

Branch: `feat/001-editor-pointer`

Writable:

- `crates/app-ui/src/editor/pointer.rs`
- `crates/app-ui/src/editor/cursor.rs`
- `crates/app-ui/src/editor/selection.rs`
- `crates/app-ui/src/editor/text_metrics.rs`
- matching docs and non-frozen tests

Tasks, in order:

- 031
- 032
- 033
- 034
- 035

One commit per task.

### Lane WORKBENCH

Branch: `feat/001-workbench`

Writable:

- `crates/app-ui/src/shell/menu.rs`
- `crates/app-ui/src/shell/explorer.rs`
- `crates/app-ui/src/shell/tabs.rs`
- `crates/app-ui/src/shell/workbench.rs`
- `crates/app-ui/src/shell/panes.rs`
- `crates/app-ui/src/shell/status.rs`
- `crates/app-ui/src/shell/theme.rs`
- matching docs and non-frozen tests

Tasks, in order:

- 040
- 041
- 042
- 043
- 044
- 045
- 046
- 047

One commit per task.

## Reviewer agents

Reviewer agents are read-only. They do not fix code.

After all implementation and top integration:

1. run requirement 071 with an explorer/reviewer agent;
2. run requirement 072 with a separate fresh reviewer;
3. run requirement 073 with a separate fresh reviewer;
4. run requirement 074 with a separate fresh adversarial reviewer.

Each reviewer writes a report under `docs/testing/requirements-001-review-*.md`.

If any report has a blocker, the top Codex assigns the fix to the owning lane or fixes a top-owned integration defect, reruns affected tests, then reruns all four reviewers from fresh contexts.

The final gate requires zero blockers from all four reviewer passes.

## Context rule

Do not spawn hundreds of tiny agents. The purpose of the small MD files is to keep each task bounded while retaining one coherent worker per ownership lane.

The top Codex keeps only orchestration, merge state, verifier results, and blockers in its main context. Verbose test output stays in worker/reviewer threads or log files.
