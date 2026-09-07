# Top Codex — runtime and interaction foundation

## Goal

Correct the root state model and freeze the interaction contracts required by Wave 3 before subagents work. This stage fixes root-only architectural defects but does not implement feature-specific rendering owned by Wave 3 branches.

## 1. Authoritative document projection

`frame_for_state` must never reconstruct the active editor with `TextBuffer::new(&state.active_text)`.

The authoritative active document is the active tab's persistent `TextBuffer`. The root projection must use its:

- snapshot,
- selections,
- version,
- dirty state,
- encoding,
- line ending.

`active_text` may remain as a compatibility/recovery projection only if every state transition keeps it synchronized. It must not be the source of truth for rendering or editor commands.

## 2. Persistent per-pane viewport state

Replace frame-local `TextViewport::default()` construction with persistent pane state.

Every editor pane stores:

- pane identifier,
- displayed tab/document identifier,
- vertical scroll origin,
- horizontal scroll origin,
- active/focused state,
- collapsed fold state.

The active cursor is kept visible after cursor movement, selection movement, search navigation, diagnostic navigation, and go-to-definition/reference navigation.

Manual scroll persists across frames until an action changes it.

## 3. Persistent fold state

Tree-sitter supplies available fold regions, but collapsed/expanded state belongs to the editor pane.

A syntax refresh updates valid fold regions without blindly expanding every previously collapsed still-valid region.

Root actions are added for:

- toggle fold at active line,
- fold selected/current region,
- unfold selected/current region,
- fold all,
- unfold all.

The root does not recreate a fresh collapsed-state set on every render.

## 4. Pane routing contract

Root input routing distinguishes editor panes by stable pane ID.

Keyboard editing targets the focused pane.

Mouse hit testing resolves the clicked editor pane before cursor/selection operations.

A tab can be shown in more than one pane without duplicating its text buffer. Editing either view changes the same document model, while viewport and fold state remain pane-specific.

## 5. Status projection

`EditorStatusData` is populated from the authoritative focused pane/document.

It must contain correct:

- cursor line,
- cursor column,
- selection count/length summary,
- file name,
- dirty state,
- encoding,
- EOL,
- indent mode/size,
- language,
- Git branch,
- diagnostic counts,
- LSP state,
- workspace trust state.

Do not leave cursor/selection data at `EditorStatusData::default()`.

## 6. Generic modal/picker contract

Extend the root interaction state so commands can open a generic selectable overlay without feature-specific ad-hoc `InputMode` variants.

The contract supports:

- title,
- optional query field,
- selectable rows,
- disabled rows,
- currently selected row,
- keyboard navigation,
- mouse selection,
- accept,
- cancel,
- typed payload returned on accept.

This contract is used later for encoding, EOL, recent workspace, branch/stash selection where appropriate, and similar finite choices.

The top Codex creates only the shared root contract and placeholder integration. Visual implementation belongs to `39_GENERIC_PICKER_UX.md`.

## 7. Command registration contract

Create one authoritative command registry. A command entry includes:

- command ID,
- user-visible title,
- optional default keybinding,
- availability predicate,
- typed action factory or typed routing target.

The Command Palette and resolved keybindings use the same registry.

It is forbidden for a command to appear in the palette without an executable route, or for an MVP user action to exist only as an internal `Action` variant with no command/mouse/keyboard route where a direct UI control is not provided.

## 8. Tests

Add deterministic root tests proving:

- rendering reads the active persistent buffer selections instead of a reconstructed buffer,
- cursor movement changes rendered cursor/status position,
- scroll state survives multiple frames,
- moving below the viewport scrolls to keep the cursor visible,
- fold collapse survives unrelated frames and syntax refresh,
- two panes can view the same document with independent viewport state,
- mouse pane targeting changes focused pane,
- command registry entries resolve to executable routes,
- disabled commands do not execute.

## Acceptance criteria

Stage 32 is complete only when:

- no production frame construction creates a replacement active `TextBuffer` from `active_text`,
- no production frame construction resets editor viewport state to default each frame,
- focused pane is explicit root state,
- fold collapsed state is persistent,
- status cursor/selection fields are derived from the active persistent buffer,
- the generic picker and command-registry contracts compile,
- all pre-existing workspace tests and new foundation tests pass,
- format and Clippy with denied warnings pass,
- source documentation mirrors are updated.
