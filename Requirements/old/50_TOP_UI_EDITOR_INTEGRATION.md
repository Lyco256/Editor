# Top Codex — UI/editor stabilization integration

## Goal

Wire Requirements 44–49 into the real runtime and remove the old competing input/render paths.

The top Codex owns root state/runtime/shared contracts/registrations and cross-feature tests.

## 1. Scene lifecycle

`AppRuntime` maintains:

- current non-zero terminal size,
- last successfully presented `WorkbenchLayoutSnapshot`.

Every render produces one scene from the current state and size.

After successful presentation, the runtime stores that scene's layout snapshot.

Mouse input is translated through this stored snapshot.

## 2. Resize integration

On `InputEvent::Resize`:

- update `AppRuntime::size`,
- render immediately using the new dimensions,
- replace last layout snapshot,
- do not forward Resize as an ordinary no-op editor action.

The status bar is always placed from the new layout and ends at `rows - 1` whenever it is visible.

Repeated resize events do not require any subsequent keyboard/mouse input to make the UI catch up.

## 3. Remove obsolete fixed-coordinate mouse paths

Delete the root logic that independently checks:

- Explorer fixed columns,
- fixed split half coordinates,
- fixed tab widths,
- fixed global editor offsets,
- fixed Git panel rows.

Use translated typed pointer targets only.

No obsolete compatibility branch may remain.

## 4. Menubar integration

Create root menu state and route it through the command registry.

Menu focus participates in the global focus model.

While a menu is open:

- editor typing does not mutate source text,
- primary hardware editor cursor is hidden,
- command availability reflects current app state.

F10 and mouse behavior use the UI actions from Requirement 44.

## 5. Explorer integration

Project `ExplorerProjection::is_directory` into the explicit shell entry kind.

Route:

- file single click -> Preview,
- file double click / Enter -> Open,
- directory expansion,
- Space -> Preview,
- Alt+Click -> open to side.

Explorer wheel events modify Explorer scroll only.

## 6. Tab state integration

Extend root tab state with:

- Preview,
- Open,
- Pinned disposition.

The state machine exactly follows Requirement 45.

Each pane/editor group owns its visible tab group state.

Do not display one global tab bar for multiple groups.

Promote Preview -> Open on first document modification before that Preview can be replaced.

Implement commands:

- Keep Editor Open,
- Pin Editor,
- Unpin Editor.

Session/recovery serialization preserves tab disposition where compatible with the current session format. Old sessions without the field load as Open.

## 7. Editor pointer integration

For an editor text target, call the Requirement 46 mapper with:

- target pane ID,
- actual text rectangle size/local coordinates,
- that pane's viewport,
- that pane's displayed document snapshot.

Apply resulting actions to the correct document/pane.

Required user behavior:

- normal click places caret,
- right whitespace -> EOL,
- below text -> EOF,
- Shift-click extends,
- Alt-click adds cursor,
- double-click selects word,
- triple-click selects line,
- drag selects with clamping,
- wheel scrolls correct pane.

Clicking an editor pane focuses it before editing actions.

## 8. Caret integration

Stop framebuffer cursor-glyph rendering.

Scene generation derives primary caret screen position from the focused pane.

When valid and visible:

- request visible `SteadyBar`.

Otherwise:

- request hidden terminal cursor.

Secondary cursors remain framebuffer style-only cursors.

## 9. Navigation/keybinding integration

Route mature editor commands from Requirement 47.

Required default Windows/Linux bindings where terminal input supports them:

- arrows / Shift+arrows,
- Home / End,
- Ctrl+Home / Ctrl+End,
- Ctrl+Left / Ctrl+Right,
- Ctrl+Backspace / Ctrl+Delete,
- PageUp / PageDown,
- Ctrl+Up / Ctrl+Down,
- Tab / Shift+Tab,
- Alt+Up / Alt+Down,
- Shift+Alt+Up / Shift+Alt+Down,
- Ctrl+Enter,
- Ctrl+Shift+Enter,
- Ctrl+L,
- Ctrl+A.

If a terminal cannot distinguish one chord, the command remains in Command Palette and receives the established conflict-free terminal fallback binding. Do not silently remove the command.

## 10. Automatic cursor reveal

After keyboard navigation/editing:

- focused pane viewport scrolls just enough to keep the active primary caret visible,
- horizontal viewport also follows when caret leaves the visible text columns.

Mouse-wheel scrolling does not immediately snap back to the cursor unless a later cursor-moving command requests reveal.

## 11. Theme integration

Use the semantic pairs from Requirement 48 throughout:

- menu,
- Explorer,
- tabs,
- editor,
- selection,
- panel,
- status,
- inputs,
- separators.

Do not use `StatusBar` or `Panel` as generic backgrounds for unrelated active tabs/menu states.

## 12. UI chrome glyph cleanup

Replace forbidden decorative glyphs in production chrome.

Examples that must disappear from chrome:

- `▸`
- `▾`
- `›`
- `•`
- `…`
- `▌`

Use central approved constants/helpers.

Source/document/user content is exempt.

## 13. Tab and Explorer hit correctness

Mouse routing uses exact layout-snapshot identities.

Tests must prove variable display-width tab titles still select the exact rendered tab.

Explorer row click must continue to target the same entry after:

- terminal resize,
- Explorer scroll,
- menu open/close,
- bottom-panel toggle.

## 14. Pointer interactions in splits

Every editor pointer event carries pane ID.

The same global coordinate after changing split ratio may map to a different pane only if the newly rendered snapshot says so.

No split decision uses half-screen constants.

## 15. Scroll integration

Consume `MouseAction::ScrollLines` according to target:

- editor text/gutter/line-number -> that pane viewport,
- Explorer -> Explorer list,
- bottom panel -> panel list where scrollable,
- menu -> menu list where scrollable,
- overview ruler -> its own navigation semantics.

Scroll is not silently dropped.


## 16. Accurate Git line markers

Delete the file-level marker projection that creates one `MarkerSpan` covering `0..buffer.len_chars()` for a changed file.

For the active document, obtain the current Git diff model and derive editor line markers from actual `GitDiffHunk` data.

Required mapping:

- addition lines in the new file -> `GitAdded`,
- modified replacement regions -> `GitModified`,
- pure deletions where no new-file line exists -> `GitDeleted` anchored to the nearest surviving new-file line/gutter position,
- untracked text file -> its existing lines may be treated as `GitAdded`,
- conflict status remains a diagnostic/conflict indication and must not cause every document line to be marked as Error.

Use the hunk `new_range` and parsed line kinds to advance old/new line counters correctly. Do not infer line numbers from diff display text.

Refresh active-document Git markers when:

- Git status/diff refresh completes,
- the active document changes,
- a save changes the working-tree diff,
- stage/unstage/discard changes the diff.

Marker calculation does not block the UI update path.

## 17. Exact workspace-search marker identity

Preserve the exact `workspace-core::SearchHit.byte_range` when converting a backend search hit into the workspace UI/root search-result model.

A project-search result also carries enough source identity to determine whether that byte range still describes the currently displayed buffer.

For a clean active buffer that still matches the searched on-disk content:

- convert exact byte-range boundaries to editor character offsets,
- paint that exact occurrence.

For a dirty or otherwise changed active buffer:

- validate the hit against the current buffer before painting,
- if exact validation fails, omit the stale overview/text marker,
- do not relocate it by searching for the first equal substring.

Opening/navigating to a search result still uses the result's exact stored location and then reveals the validated match/line.

Delete the runtime fallback that uses `line_text.find(matched_text)` to reconstruct result position.

## 18. Contextual overlay screen anchoring

Contextual language overlays are positioned from the actual focused pane layout, not directly from document line/character integers.

Mapping order:

1. identify the document/pane that owns the overlay request,
2. reject the overlay if its document/version is stale,
3. convert document logical position to document display column,
4. subtract pane vertical/horizontal viewport origins,
5. add the actual pane text-rectangle screen origin,
6. place/flip/clip the overlay inside the current terminal layout.

If the document anchor is currently scrolled out of view, dismiss the ephemeral cursor-context overlay instead of pinning it to an unrelated screen edge.

The same mapping is recalculated after resize and pane layout changes.

## 19. Grapheme-safe overlays and chrome

Replace root overlay text loops that advance one column per Rust `char` with the shared grapheme/display-cell writer from Requirement 43.

This includes:

- keyboard input overlays,
- contextual LSP overlays,
- transient prompts created by root integration.

CJK or combining text must not overwrite borders or adjacent columns.


## 20. Effective indentation/formatting configuration

Delete the root LSP-formatting request path that hard-codes `tabSize = 4` and `insertSpaces = true`.

Every formatting request uses the active document's effective:

- tab width,
- insert-spaces setting.

The same effective tab width is passed to:

- vertical preferred-column calculations,
- pointer display-column mapping,
- Home/End/word navigation helpers where display width matters,
- indentation/outdent,
- formatter/LSP formatting options.

Tests change the effective settings to non-default values and prove the request/movement output changes accordingly.

## 21. Document identity for language results

Document-scoped LSP result projection preserves the `DocumentId` associated with the request/response.

Replace fixed `DocumentId(1)` in inlay-hint construction with the actual response/request document.

Apply the same invariant to:

- completion context,
- hover,
- signature help,
- diagnostics,
- semantic tokens,
- inlay hints,
- rename/contextual overlays,
- go-to/reference results where an owning/source document exists.

Before painting/applying a document-scoped result, validate both document identity and document version.

## 22. One contextual language presentation path

Remove the dependency where contextual language actions force `BottomPanelView::Language` open.

The bottom panel continues to host:

- Problems,
- Output,
- Search,
- Git.

Cursor-context language features use the contextual overlay state only:

- completion,
- completion details,
- hover,
- signature help,
- code actions / Quick Fix,
- rename,
- location/reference chooser where contextual.

Accept/dismiss/navigation actions operate on that overlay state.

No second generic Language panel is required for those operations.

If `BottomPanelView::Language` becomes unused after migration, remove it and its dead renderer/state rather than keeping a parallel fallback.

## Cross-feature automated tests

Add deterministic integration tests for:

### Git/search marker accuracy

- a one-line Git edit in a 100-line file marks only the hunk/affected anchor lines, never the full document,
- add/modify/delete hunk fixtures produce the correct marker kinds,
- two identical search matches on the same line produce two distinct exact markers,
- a dirty-buffer stale project-search hit is not silently moved to the first equal substring.

### Contextual overlay geometry

- completion anchor follows the caret after vertical scroll,
- completion anchor follows the caret after horizontal scroll,
- Explorer toggle changes screen x while preserving document anchor,
- secondary split pane produces an anchor inside that pane,
- resize recomputes the screen anchor,
- off-screen/stale anchor dismisses the ephemeral overlay,
- CJK overlay labels preserve borders.

### Resize

Sequence:

- render 120x40,
- resize 80x24,
- resize 160x50,
- resize 55x12,
- resize back to 120x40.

After each size:

- framebuffer has exact size,
- status bottom row is correct if visible,
- every layout rectangle is within frame,
- click center of every visible major region resolves to that rendered region,
- no stale old-size click geometry remains.

### Editor click clamping

For `abcde\n\nxyz`:

- click right of `abcde` -> first-line EOL,
- click any horizontal position on blank second line -> second-line EOL,
- click below `xyz` -> EOF.

### Preferred column

For `abcde\n\nabcde`:

- line1 col5 -> Down -> blank col0 with goal5 -> Down -> line3 col5.

### Tab

- single-click Explorer file -> Preview,
- another single click replaces clean Preview,
- editing Preview promotes to Open,
- double-click opens Open,
- Pin produces Pinned,
- exact rendered tab click selects matching tab after resize.

### Editing configuration and grapheme deletion

- effective tab width 2/8 changes vertical display-column mapping correctly,
- LSP formatting request uses active `tab_width` and `insert_spaces`,
- Delete removes an entire combining grapheme/emoji grapheme, not one scalar value,
- Home/End/PageUp/PageDown required routes are not silent no-ops.

### Language document identity

- inlay hints from document B retain document B ID rather than `DocumentId(1)`,
- stale/wrong-document hint/overlay is not painted,
- contextual completion/rename/code-action flow does not open a generic Language bottom panel.

### Cursor/selection

- framebuffer source cell under primary caret remains source grapheme,
- selection preserves text,
- terminal adapter records SteadyBar at expected cell,
- secondary cursor preserves text.

### Menu

- F10 focus,
- File menu navigation,
- mouse open/close,
- menu open hides editor caret.

## Acceptance criteria

Requirement 50 is complete only when:

- resize is live and immediate,
- render and pointer geometry have one source of truth,
- no fixed-coordinate root hit logic remains,
- Git markers come from actual diff hunk coordinates rather than file-level whole-buffer spans,
- workspace-search markers preserve exact backend match identity and never use first-substring reconstruction,
- contextual overlays map document coordinates through the actual pane viewport/layout,
- document-scoped LSP results preserve real document identity/version,
- contextual LSP actions no longer depend on a generic Language bottom panel,
- effective indentation settings are used by navigation and formatting,
- root overlay text is grapheme/display-cell safe,
- menu/Explorer/tabs/editor groups use the new workbench,
- preview/open/pinned semantics are real root state,
- all required pointer/editor navigation semantics are reachable,
- the old cursor glyph is gone from chrome,
- scroll events are consumed by target widgets,
- all integration tests, docs mirrors, format, Clippy, and workspace tests pass.
