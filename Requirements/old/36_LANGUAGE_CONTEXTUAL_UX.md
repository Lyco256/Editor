# Subagent requirement — contextual language UX

## Branch

`feat/language-context-ux`

## Writable ownership

- `crates/app-ui/src/language/**`
- `docs/crates/app-ui/src/language/**`
- `tests/fixtures/app-ui/language/**`
- language-specific snapshots under `tests/snapshots/**`

Do not edit root `src/app/**`, `crates/app-ui/src/editor/**`, shared types, other UI feature directories, or `Cargo.lock`.

## Goal

Replace bottom-panel-only presentation for cursor-context language features with terminal-native contextual overlays while preserving the Problems panel for diagnostics.

## 1. Contextual overlay model

Create pure view models/renderers for overlays anchored to an editor cursor/screen position.

Required overlays:

- completion list,
- completion details,
- hover card,
- signature help,
- code action / Quick Fix list,
- rename input,
- go-to location chooser when more than one target exists,
- references chooser,
- document symbol chooser,
- workspace symbol chooser.

The overlay placement algorithm:

- prefers below/right of the cursor,
- flips above/left when required to stay on screen,
- clips content only after attempting alternate placement,
- never writes outside framebuffer bounds,
- leaves at least one row/column of active editor context when terminal size allows it.

## 2. Completion interaction

Completion state supports:

- selected item,
- next/previous,
- page movement,
- accept,
- cancel,
- optional resolve/details request for selected item.

The accepted item preserves the LSP edit payload/identity needed by root integration. It is not reduced to label text only.

## 3. Hover and signature behavior

Hover and signature help do not force the bottom Problems/Output area open.

They close on:

- Escape,
- cursor movement that invalidates their request context,
- document version change when the result is stale,
- explicit replacement by another contextual overlay.

## 4. Code actions and rename

Code-action rows preserve the executable action/workspace-edit payload identifier required by root routing.

Rename has an editable input state and a preview/result state. Enter submits the new name; Escape cancels without applying edits.

## 5. Inlay hint data

`InlayHintsView` must preserve individual hint positions and labels rather than only a generic panel list.

It exposes an ordered collection consumable by root integration and the editor's inline-hint primitive.

The language module does not itself modify source text.

## 6. Problems remains a panel

Diagnostics/Problems stay in the bottom panel and continue to support grouping and navigation.

Completion, hover, signature help, inlay hints, and Quick Fix are not represented solely by switching the bottom panel to a generic "Language" panel.

## Tests

Deterministic tests cover:

- overlay below/right placement,
- edge flipping,
- tiny-terminal clipping,
- completion navigation and payload identity,
- completion details,
- hover dismiss,
- signature dismiss,
- code-action selection,
- rename input/cancel/submit state,
- location chooser,
- inlay hint positions,
- stale result suppression,
- Unicode labels.

## Acceptance criteria

- Cursor-context language results have contextual overlay renderers.
- Completion items retain enough structured identity for root to accept the selected actual LSP item.
- Inlay hints retain document positions.
- Context overlays do not require the bottom panel.
- All owned tests, Clippy, formatting, and docs mirrors pass.
