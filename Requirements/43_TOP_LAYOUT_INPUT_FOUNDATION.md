# Top Codex — authoritative layout and input foundation

## Goal

Create one authoritative geometry/presentation contract before parallel UI repair begins.

Rendering, mouse hit testing, pointer-to-document conversion, tab targeting, split targeting, menu targeting, and caret placement must all consume the same layout result.

## 1. Render scene contract

Replace the concept of “framebuffer only” root rendering with a scene result that contains:

- framebuffer,
- authoritative `WorkbenchLayoutSnapshot`,
- optional terminal cursor presentation.

The exact Rust names may differ, but the three values are one coherent output of the same state/size projection.

A root render must never calculate layout once for drawing and independently recalculate guessed geometry for input.

## 2. WorkbenchLayoutSnapshot

Create a typed immutable layout snapshot that records the rectangles/regions actually rendered for the current frame.

It includes at minimum:

- full terminal rectangle,
- top menu bar,
- Explorer title/body when visible,
- each editor group ID,
- each editor group tab strip,
- each editor group content rectangle,
- each editor group line-number region,
- each editor group gutter region,
- each editor group text region,
- each editor group overview-ruler region,
- split separators/handles,
- bottom panel region,
- status bar,
- active overlay/modal/menu regions,
- exact tab rectangles with tab identity,
- exact Explorer visible-row rectangles with entry identity.

Coordinates are screen-cell coordinates.

All child rectangles are contained by the terminal rectangle.

Non-overlay regions do not overlap except at deliberately shared separator boundaries.

## 3. Runtime resize handling

`InputEvent::Resize { columns, rows }` is intercepted by the interactive runtime before normal application input dispatch.

For a resize where both dimensions are greater than zero:

1. set the runtime terminal size to the new dimensions,
2. invalidate the stored layout snapshot,
3. render a new scene using the new dimensions,
4. present that new frame,
5. store its layout snapshot for subsequent pointer events.

A resize to zero in either dimension is ignored until a later non-zero resize rather than constructing an invalid framebuffer.

`AppState` must not be the owner of the physical terminal size.

Resize is no longer a silent no-op.

## 4. Pointer translation boundary

Raw terminal mouse screen coordinates are translated by the runtime/UI boundary using the most recently presented `WorkbenchLayoutSnapshot`.

`AppState` receives a typed target/local-coordinate event rather than deriving workbench regions from raw screen constants.

Typed pointer targets include at minimum:

- menu bar item/menu item,
- Explorer row,
- editor group tab,
- editor group line-number area,
- editor group gutter/fold area,
- editor group text area,
- editor group overview ruler,
- split handle,
- bottom panel,
- status bar,
- overlay/picker/palette,
- outside/no target.

Editor text targets include:

- pane/group ID,
- coordinate relative to text rectangle,
- original screen coordinate,
- modifier state,
- mouse action,
- click count.

## 5. Hard-coded geometry prohibition

Remove production mouse routing that directly uses constants to identify major UI areas.

The following patterns must not remain as workbench-routing logic:

- fixed 120x40 hit-test frame,
- Explorer `column < 24`,
- fixed subtraction by 25 or 1 to locate editor text,
- split decision at column 60 / row 20,
- fixed tab index by `column / constant`,
- Git/action row thresholds independent of the actual panel rectangle.

Feature-specific minimum sizes remain allowed inside the layout engine, but input code may not use them as global screen boundaries.

## 6. Shell/editor module split for worktree ownership

Mechanically split the current monolithic shell/editor files before feature branches start.

Required shell facade structure:

- `shell/mod.rs` — top-owned public facade/registrations only
- `shell/geometry.rs` — top-owned snapshot/hit types
- `shell/workbench.rs` — Wave 4A workbench renderer
- `shell/menu.rs` — Wave 4A menu renderer/state
- `shell/explorer.rs` — Wave 4A Explorer presentation
- `shell/tabs.rs` — Wave 4A tab presentation
- `shell/panes.rs` — Wave 4B pane/split presentation

Required editor facade structure:

- `editor/mod.rs` — top-owned public facade/registrations only
- `editor/viewport.rs` — existing text/marker viewport rendering
- `editor/pointer.rs` — Wave 4B pointer mapping
- `editor/cursor.rs` — Wave 4B caret presentation
- `editor/selection.rs` — Wave 4B selection/secondary-cursor presentation

The mechanical extraction must preserve existing tests before behavior changes.


## 7. Text coordinate-domain invariant

Character offsets, byte offsets, logical line numbers, logical character columns, display-cell columns, and screen coordinates are distinct domains.

Production code must not compare or cast between these domains merely because their underlying integer types are compatible.

In particular:

- a `TextRange` character offset is converted through the current `TextSnapshot` before it can become a logical line span,
- a workspace-search byte range is converted through the exact text version it belongs to,
- a document `LogicalPosition` is converted through the target pane viewport/layout before it can become a screen coordinate,
- display columns are calculated through grapheme/display-width helpers.

Remove any marker helper that has a mode in which character offsets are compared directly with line numbers.

Add typed helper APIs so the safe conversion is the shortest normal implementation path.

## 8. Shared framebuffer text-writing contract

Expose/reuse one grapheme-aware clipped text writer for all application chrome and overlay text.

No production overlay/menu/picker/status/tab writer may advance terminal x by `.chars().enumerate()` or assume one Rust `char` equals one terminal cell.

The writer:

- segments grapheme clusters,
- uses terminal display width,
- never starts a write on a continuation cell,
- truncates without splitting a grapheme,
- clips safely at frame bounds.

## 9. Display-cell metric contract

Expose one shared terminal display-width implementation based on the same grapheme segmentation and Unicode width logic already used by the framebuffer.

At minimum expose helpers equivalent to:

- grapheme display width,
- string display width,
- truncate to maximum display cells without splitting a grapheme,
- map display column to a safe string/document boundary where needed.

Shell/workbench screen layout must not use `chars().count()` as display width.

Document logical character offsets remain separate from display-cell width.

## 10. Chrome glyph policy

Define the allowed structural UI glyph policy centrally.

UI chrome may use:

- printable ASCII U+0020–U+007E,
- Unicode Box Drawing U+2500–U+257F.

UI chrome must not depend on:

- emoji,
- Nerd Font symbols,
- Powerline glyphs,
- geometric arrows/triangles,
- bullets,
- decorative single-character ellipsis.

User content, file names, source code, diagnostic text, completion labels, and other user/data strings remain fully Unicode.

Required replacements include:

- `…` -> `...`
- `•` -> ` | `
- geometric expand/collapse icons -> ASCII `[+]` / `[-]` or Box Drawing plus ASCII markers
- custom graphical caret glyph -> no framebuffer glyph caret

Create an automated source-policy check for application chrome constants.

## 11. Shared semantic style roles

Expand top-owned `StyleRole` before Wave 4A so features do not independently add enum variants.

Required distinct roles include:

- EditorText
- EditorBackground
- CurrentLineBackground
- SelectionForeground
- SelectionBackground
- SecondaryCursorForeground
- SecondaryCursorBackground
- LineNumber
- LineNumberActive
- Gutter
- SplitSeparator
- MenuForeground
- MenuBackground
- MenuSelectedForeground
- MenuSelectedBackground
- ExplorerForeground
- ExplorerBackground
- ExplorerDirectory
- ExplorerSelectedForeground
- ExplorerSelectedBackground
- TabForeground
- TabBackground
- TabActiveForeground
- TabActiveBackground
- TabPreviewForeground
- TabPinnedForeground
- PanelForeground
- PanelBackground
- PanelTitle
- InputForeground
- InputBackground
- StatusForeground
- StatusBackground
- Border
- Error
- Warning
- Information
- Hint
- GitAdded
- GitModified
- GitDeleted
- SearchMatch
- SyntaxKeyword
- SyntaxString
- SyntaxComment
- SemanticType

Existing code is migrated from coarse old roles to the nearest correct new role.

## 12. Terminal cursor presentation contract

Expose cursor presentation through the generic terminal abstraction.

The runtime can request:

- hidden cursor,
- visible cursor at a screen cell,
- `SteadyBar` as the normal focused-editor shape.

The concrete crossterm backend remains responsible for emitting terminal commands.

A fake terminal adapter records cursor presentation for deterministic tests.

## 13. Multi-click normalized input contract

Add click-count information to normalized mouse input.

The production input layer derives:

- single click,
- double click,
- triple click

from consecutive left-button click sequences at the same cell using a 500 ms maximum interval between clicks.

The click tracker uses an injectable clock in tests.

A drag resets click-chain interpretation.

Mouse-wheel line delta remains an explicit `ScrollLines(i16)` action.

## Tests

Stage 43 tests prove:

- resize 120x40 -> 80x24 -> 160x50 updates the actual render size each time,
- status rectangle always ends on the last row of each non-zero frame,
- the same layout snapshot used to render a region identifies a click in that region,
- no hit test internally substitutes 120x40,
- exact variable-width tab rectangles hit the correct tab,
- a grapheme-width helper agrees with framebuffer ownership for ASCII, CJK, emoji user content, and combining sequences,
- chrome-policy checker rejects forbidden decorative glyph fixtures,
- fake terminal records `SteadyBar` cursor requests,
- multi-click tracker produces 1/2/3 and resets after timeout/drag,
- a character-offset marker on a multi-line document maps through `TextSnapshot` to the correct logical line,
- grapheme-aware shared text writing keeps CJK and combining text inside its allocated rectangle.

## Acceptance criteria

Stage 43 is complete only when:

- runtime size changes on resize,
- one authoritative layout snapshot exists,
- raw global mouse coordinates are no longer interpreted using hard-coded workbench constants in `AppState`,
- shell/editor files are split into the ownership-safe structure,
- display-cell width helpers and the shared clipped text writer are shared,
- unsafe direct comparisons between character offsets and logical line numbers are removed,
- style roles are frozen for this continuation,
- cursor and click-count contracts compile,
- all repository tests, formatting, Clippy, and docs mirror checks pass.
