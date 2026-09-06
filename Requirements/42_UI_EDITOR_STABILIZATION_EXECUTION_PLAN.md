# UI/editor stabilization execution plan

## Purpose

This continuation follows Requirements 00–41.

Its purpose is to correct the remaining workbench geometry, resizing, pointer, caret, selection, navigation, tab, Explorer, theme, and information-density defects found in the latest `devenv` implementation.

A feature is not complete because its visual output looks plausible at one terminal size. The same layout geometry must drive rendering and input, and automated tests must prove behavior across resize, Unicode, compact layouts, split panes, and editor navigation edge cases.

## Automated-only completion policy

Completion uses repository-local automated tests and deterministic fake-terminal tests.

The continuation does not require:

- a human to resize a real window,
- a human to visually inspect Windows Terminal,
- installer execution,
- code signing,
- elevated privileges,
- remote GitHub administration,
- external network services.

## Execution order

### Stage 1 — top foundation

The top Codex completes:

- `43_TOP_LAYOUT_INPUT_FOUNDATION.md`

No feature worktree starts until Stage 43 is green.

### Stage 2 — Wave 4A

Create these branches/worktrees from the exact tested Stage 43 `devenv` commit:

- `feat/workbench-menubar` → `44_WORKBENCH_LAYOUT_MENUBAR.md`
- `feat/explorer-tabs-ux` → `45_EXPLORER_TAB_UX.md`
- `feat/editor-navigation` → `47_VSCODE_LIKE_EDITOR_NAVIGATION.md`
- `feat/theme-terminal-presentation` → `48_THEME_CONTRAST_TERMINAL_PRESENTATION.md`

The top Codex merges Wave 4A into `devenv`, regenerates `Cargo.lock`, and returns the complete repository to green.

### Stage 3 — Wave 4B

Create these branches/worktrees from the exact tested Wave 4A integration commit:

- `feat/editor-pointer-selection` → `46_EDITOR_POINTER_CURSOR_SELECTION.md`
- `feat/pane-density` → `49_PANE_GEOMETRY_INFORMATION_DENSITY.md`

The top Codex merges Wave 4B into `devenv`, regenerates `Cargo.lock`, and returns the repository to green.

### Stage 4 — top integration

The top Codex completes:

- `50_TOP_UI_EDITOR_INTEGRATION.md`

### Stage 5 — final automated gate

The top Codex completes:

- `51_AUTOMATED_UI_EDITOR_REGRESSION_ACCEPTANCE.md`

## Ownership

The existing ownership rules remain in force.

During this continuation, only the top Codex edits:

- `src/app/**`
- `crates/editor-types/**`
- root `Cargo.toml`
- `Cargo.lock`
- `crates/app-ui/src/lib.rs`
- `crates/app-ui/src/shell/mod.rs`
- `crates/app-ui/src/editor/mod.rs`
- cross-feature integration tests
- central command registration

Feature branches edit only the files explicitly assigned in their requirement.

## No hidden fallback to old geometry

Old fixed-coordinate paths are not retained as compatibility fallbacks.

After the new layout snapshot is integrated, production pointer routing must not contain parallel logic that assumes 120x40, Explorer width 24/25, split column 60, split row 20, or fixed tab widths.

## Completion definition

This continuation is complete only when Requirements 43–50 are integrated into `devenv`, audit findings A-01 through A-27 each have passing automated proof, and every gate in Requirement 51 passes.

A partially corrected UI that works at one terminal size, has correct rendering but incorrect mouse targeting, or passes snapshots while preserving fixed-coordinate fallback paths is not complete.
