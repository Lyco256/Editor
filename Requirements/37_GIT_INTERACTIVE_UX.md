# Subagent requirement — interactive Git UX

## Branch

`feat/git-interactive-ux`

## Writable ownership

- `crates/app-ui/src/git/**`
- `docs/crates/app-ui/src/git/**`
- `tests/fixtures/app-ui/git/**`
- Git-specific snapshots under `tests/snapshots/**`

Do not edit root `src/app/**`, `vcs-git`, shared types, other UI feature directories, or `Cargo.lock`.

## Goal

Make the existing Git dashboard fully operable through typed keyboard/mouse actions instead of being primarily a renderer with internally callable mutation actions.

## 1. Focus and selection model

The Git UI has explicit focus targets for:

- view/sidebar,
- changed-file list,
- diff/hunk list,
- commit form,
- branch list,
- stash list,
- history list,
- confirmation dialog.

Typed input actions support:

- focus next/previous region,
- list up/down,
- page up/down,
- activate,
- cancel/back.

Mouse hit testing selects the same rows/actions represented by keyboard focus.

## 2. Changes and diff actions

From the UI a user can trigger:

- open diff,
- stage selected file,
- unstage selected file,
- stage selected hunk,
- unstage selected hunk,
- request discard,
- confirm discard,
- cancel discard,
- toggle working-tree/index view.

Actions preserve exact path/hunk payloads and never rely on display text parsing.

## 3. Commit form

The commit form supports:

- editable commit message,
- amend toggle,
- submit,
- cancel/clear.

Commit is unavailable with an invalid/empty form unless the existing backend request explicitly permits the selected mode.

No author credential editor is added.

## 4. Branches

User-operable typed actions exist for:

- select branch,
- switch branch,
- create branch by terminal-native text entry,
- delete selected branch with confirmation.

Force deletion requires a separately explicit action/confirmation and is never the default delete behavior.

## 5. Fetch, pull, push

The Git UI exposes refresh, fetch, pull, and push actions.

Busy state prevents duplicate submission of the same mutating/network command while it is still pending.

Failures remain visible through the existing operation/output model.

## 6. Stashes and history

The UI supports:

- select stash,
- create stash with optional message,
- apply selected stash,
- pop selected stash with confirmation,
- select history entry,
- inspect basic history details.

## 7. Trust

When Workspace Trust disables Git, mutation and process-launch actions are disabled in the interaction model, not merely rejected after selection.

## Tests

Pure Git UI tests cover:

- keyboard navigation through each view,
- mouse selection,
- stage/unstage exact payload,
- hunk stage/unstage exact payload,
- discard confirm/cancel,
- commit message editing,
- amend,
- branch create/switch/delete confirmation,
- fetch/pull/push busy-state suppression,
- stash apply/pop,
- untrusted disabled actions,
- narrow layout.

No test contacts a network remote.

## Acceptance criteria

- Every Git operation required by the original MVP has a user-operable typed UI route.
- No Git mutation requires a hidden direct `GitAction` call unavailable from normal interaction.
- Destructive operations remain confirmed.
- UI does not construct shell commands.
- All owned tests, Clippy, formatting, and docs mirrors pass.
