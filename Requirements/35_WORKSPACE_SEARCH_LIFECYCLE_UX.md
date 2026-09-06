# Subagent requirement — workspace search and lifecycle UX

## Branch

`feat/workspace-search-ux`

## Writable ownership

- `crates/app-ui/src/workspace/**`
- `docs/crates/app-ui/src/workspace/**`
- `tests/fixtures/app-ui/workspace/**`
- workspace-specific snapshots under `tests/snapshots/**`

Do not edit root `src/app/**`, `workspace-core`, shared types, other UI feature directories, or `Cargo.lock`.

## Goal

Complete the user-facing workspace/search models that the current backend capabilities require.

## 1. Project search options

Extend the workspace search UI model to expose:

- regex vs literal mode,
- case sensitivity,
- whole-word mode,
- include glob/filter,
- exclude glob/filter,
- result limit.

The model must represent the exact options needed by `workspace-core`; do not encode options only in display strings.

The search view shows active options and allows the root to update each option through typed UI actions.

## 2. Project replace workflow

The workspace UI supports:

1. query entry,
2. replacement text entry,
3. search result population,
4. replacement-plan preview,
5. count of affected files and replacements,
6. explicit confirm,
7. cancel,
8. completion/failure summary.

Confirmation is required before a multi-file replacement plan is applied.

A replacement failure summary distinguishes files successfully written from files not written.

## 3. Search result interaction

Typed UI actions exist for:

- move selection up/down,
- page up/down,
- open selected result,
- open selected result to side,
- cancel active search,
- restart search after option change.

Keyboard and mouse selection use the same selected-result state.

## 4. Recent workspace model

Add a user-visible recent-workspace list model with:

- canonical path,
- display label,
- recency order,
- missing-path state.

Typed actions exist for select, open, and remove from recent list.

The UI never silently deletes a recent entry merely because the path is temporarily unavailable.

Persistence and root routing are handled by the top integration stage.

## 5. File lifecycle prompts

Provide typed prompt/view models usable by root commands for:

- New File,
- Open File by path,
- Open Folder/Add Workspace Folder by path,
- Save As,
- Close Editor with dirty-buffer confirmation.

No native GUI file dialog is required. These are terminal-native path entry/selection flows.

## Tests

Framebuffer/model tests cover:

- every search option,
- include/exclude display and typed state,
- replace preview before confirm,
- replace cancel,
- result keyboard/mouse selection,
- open-to-side action,
- recent workspace ordering,
- missing recent path rendering,
- New/Open/Save As/Close prompt states,
- compact terminal layout.

## Acceptance criteria

- Project search exposes every original MVP search option.
- Project replacement has an explicit preview and confirm state.
- Recent workspaces are represented as an actual user-visible model.
- File lifecycle prompts are terminal-native and require no platform GUI dialog.
- All owned tests, Clippy, formatting, and docs mirror checks pass.
