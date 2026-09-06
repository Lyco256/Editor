# MVP gap-closure execution plan

## Purpose

These requirements continue the existing `00` through `30` requirements. They do not redefine features that are already correctly implemented.

The starting point is the current `devenv` implementation audited on 2026-09-06. The goal is to close remaining gaps where a backend/model exists but the user-visible behavior is incomplete, where the root runtime projects incorrect state, or where an original MVP capability is not reachable from normal keyboard/mouse interaction.

This continuation uses repository-local automated validation only. Work completion does not depend on external account actions, remote repository administration, interactive platform sign-off, or installation/signing flows.

## Required execution order

1. The top Codex completes `32_TOP_RUNTIME_INTERACTION_FOUNDATION.md` on `devenv`.
2. The complete automated verification suite must be green.
3. Wave 3 feature agents branch from that exact tested `devenv` commit and work in separate worktrees:
   - `33_EDITOR_VIEWPORT_VISUAL_CORRECTNESS.md`
   - `34_MULTI_CURSOR_EDITOR_INTERACTION.md`
   - `35_WORKSPACE_SEARCH_LIFECYCLE_UX.md`
   - `36_LANGUAGE_CONTEXTUAL_UX.md`
   - `37_GIT_INTERACTIVE_UX.md`
   - `38_SPLIT_PANE_INTERACTION.md`
   - `39_GENERIC_PICKER_UX.md`
4. The top Codex merges Wave 3 branches into `devenv`, regenerates `Cargo.lock`, and returns the workspace to green.
5. The top Codex completes `40_TOP_GAP_INTEGRATION_WIRING.md`.
6. The top Codex completes `41_AUTOMATED_MVP_GAP_ACCEPTANCE.md`.

## Branch names

- `feat/editor-viewport-visuals`
- `feat/editor-multicursor`
- `feat/workspace-search-ux`
- `feat/language-context-ux`
- `feat/git-interactive-ux`
- `feat/split-pane-interaction`
- `feat/generic-picker-ux`

All branch and ownership rules in `01_AGENT_AND_GIT_RULES.md` remain in force.

## Conflict minimization

The top Codex exclusively owns during this continuation:

- `src/app/**`
- `crates/editor-types/**`
- root `Cargo.toml`
- `Cargo.lock`
- `crates/app-ui/src/lib.rs`
- shared module-registration files
- root integration tests that exercise more than one feature crate

Feature agents do not edit these files.

Each feature branch owns disjoint crate subdirectories as stated in its requirement file.

## Completion state

The gap-closure cycle is complete when every requirement in `33` through `40` is integrated into `devenv` and every automated gate in `41` passes.

Remote branch promotion or remote push is not part of this continuation cycle.
