# Editor latest UI/editor bug audit

Audit target: `Lyco256/Editor`, branch `devenv`, reviewed against the latest repository state available on 2026-09-06.

This audit supplements the existing Requirements 00–41. It is not a replacement for them.

## Confirmed defects and incomplete behavior

### A-01 — resize events do not update the runtime render size

`src/app/runtime.rs` stores the terminal size in `AppRuntime::size` and renders every frame with that value. The interactive loop forwards `InputEvent::Resize` to `AppState`, but does not update `AppRuntime::size`.

`src/app/state.rs` currently treats `InputEvent::Resize` as a no-op.

Result: framebuffer size, status bar position, workbench layout, and subsequent pointer geometry can remain based on the old dimensions.

### A-02 — shell hit testing uses a fixed 120x40 layout

`crates/app-ui/src/shell/mod.rs::hit_test` currently calls the layout function with `Rect::new(0, 0, 120, 40)`.

Result: the geometry used for drawing and the geometry used for pointer hit testing diverge whenever the real terminal is not exactly 120x40.

### A-03 — root mouse routing contains hard-coded geometry

`src/app/state.rs` contains screen-coordinate assumptions including:

- Explorer width around 24/25 cells,
- Explorer row offset around 3,
- split selection at column 60 / row 20,
- editor text coordinate subtraction by fixed shell offsets,
- tab selection using fixed column widths,
- Git row thresholds.

Result: clicks can target the wrong UI region after resizing, compact layout changes, panel visibility changes, split changes, or variable-width tabs.

### A-04 — mouse wheel is normalized but not routed through the workbench/editor

`terminal-backend` produces `MouseAction::ScrollLines`, but shell dispatch discards the action and root editor routing does not implement pane-local wheel scrolling.

### A-05 — cursor rendering overwrites the source glyph

`crates/app-ui/src/editor/mod.rs::render_cursors` writes a `▌` glyph into the framebuffer at each cursor position.

Result: the original source character is removed visually at the caret, and selection/caret combinations can make source text unreadable.

### A-06 — selection foreground/background semantics are insufficient

Selection rendering uses one generic `StyleRole::Selection` as a foreground role while retaining the row background.

The current semantic palette does not define independent selection foreground and selection background roles.

Result: selected text can have insufficient contrast and does not reproduce conventional editor selection behavior.

### A-07 — vertical movement loses the preferred display column

`editor-core::TextBuffer::move_vertical` calculates the desired display column again from the current clamped cursor position on every call.

Example defect:

- start at line 1, display column 5,
- move Down to an empty line and clamp to column 0,
- move Down again,
- cursor stays at column 0 instead of returning to column 5 on the longer line.

### A-08 — tree/tab chrome uses width-unstable decorative Unicode

The current UI uses characters such as geometric arrows, bullets, and a single-character ellipsis. Tab sizing also uses `chars().count()`.

Result: display width can differ across terminal fonts/Unicode width rules, causing separators and hit regions to drift.

### A-09 — shell model loses explicit Explorer entry kind

Root Explorer projections know whether an entry is a directory. The shell-level `ExplorerEntry` does not carry an explicit file/directory kind.

Result: renderer behavior cannot reliably distinguish files and directories beyond expansion state.

### A-10 — preview/permanent/pinned tab state is not modeled

`TabState` and shell `TabEntry` do not contain preview/open/pinned disposition.

Result: a user cannot tell whether a single-click file preview will be replaced, whether an editor is permanent, or whether a tab is explicitly pinned.

### A-11 — no Edit-like top menu bar exists

Current shell geometry starts with a tab row. There is no persistent `File / Edit / Selection / View / Go / Help` menu bar.

Result: discoverability is below Microsoft Edit and VS Code expectations.

### A-12 — current workbench consumes space with unnecessary borders/dividers

The current pane renderer draws outer borders and full divider lines. The global tab strip also spans the workbench instead of belonging to individual editor groups.

Result: terminal information density is lower than necessary and layout does not resemble VS Code editor groups.

### A-13 — theme roles are too coarse for readable chrome

Current `StyleRole` has only generic Editor, Selection, Gutter, StatusBar, and Panel roles. The default Editor background is not black.

Result: menu, Explorer, tabs, selected rows, borders, input fields, and panels reuse colors that can blend together.

### A-14 — editor mouse-to-document mapping does not implement mature editor clamping

The current mapping returns an offset only when a screen row/column converts directly through fixed offsets.

Required mature behavior is missing or unreliable for:

- clicking to the right of line content,
- clicking below the last document line,
- drag selection beyond line ends,
- Shift-click extension,
- Alt-click multiple cursors,
- double-click word selection,
- triple-click line selection,
- gutter-specific behavior.

### A-15 — shell tab hit testing assumes fixed widths

The shell derives the tab index from a fixed column division instead of the exact tab rectangles used during rendering.

Result: clicking a variable-width tab can activate a different tab.

### A-16 — current non-whole-document marker fallback is still offset/line unsafe

The editor contains a path in which `TextRange` character offsets are converted directly to integers and compared to logical line numbers when whole-document overview mode is disabled.

Character offsets and line numbers must never share a comparison domain.

### A-17 — hardware cursor functionality exists below the public terminal abstraction only

`CrosstermBackend` already supports moving the terminal cursor, changing visibility, and selecting `SteadyBar`, but the generic `TerminalAdapter` exposed to the runtime only provides framebuffer render/enter/restore.

Result: the root renderer cannot use a native thin caret without depending on the concrete terminal backend.

### A-18 — current workbench does not have one authoritative layout snapshot

Rendering, shell hit testing, and root mouse mapping independently infer geometry.

Result: even after individual constants are corrected, later UI layout changes can reintroduce mouse/render mismatch.


### A-19 — Git editor markers mark the entire changed file instead of actual diff lines

`src/app/runtime.rs` currently turns a matching `GitStatusEntry` into one `MarkerSpan` covering character offset `0..buffer.len_chars()`.

Result: a file with a one-line edit can appear as Added/Modified across the complete document gutter/overview ruler.

The Git backend already exposes `GitDiffFile` / `GitDiffHunk` with old/new line ranges and parsed diff lines. Editor Git markers must be derived from these hunk coordinates, not file-level status.

### A-20 — workspace search UI discards the exact match byte range

`workspace-core::SearchHit` contains an exact absolute `byte_range`, but `app-ui::workspace::SearchResult` drops it.

`src/app/runtime.rs` reconstructs the active-file marker by calling `line_text.find(matched_text)`, which always chooses the first equal substring on the line.

Result: if the same matched text occurs multiple times on a line, later search results can be painted at the first occurrence. Unsaved-buffer changes can make the reconstructed location stale as well.

The exact match range/source identity must survive the backend -> UI/root projection, and stale disk-search hits must not be painted against a modified buffer without validation.

### A-21 — contextual LSP overlay anchor mixes document coordinates with screen coordinates

The root runtime currently derives popup x/y directly from `LogicalPosition.character` / `LogicalPosition.line`.

Those are document coordinates, not framebuffer coordinates. They do not include:

- menu height,
- editor-group tab height,
- Explorer width,
- line-number/gutter width,
- pane origin,
- vertical viewport scroll,
- horizontal viewport scroll,
- split-group position.

Result: completion/hover/signature/code-action overlays can appear away from the caret after scrolling, resizing, toggling Explorer, or using split panes.

### A-22 — overlay text writing assumes one Rust `char` equals one terminal cell

Some root overlay writing paths enumerate `.chars()` and increment the framebuffer column by one for each `char`.

Result: CJK, emoji/data strings, and combining sequences can overwrite neighboring cells or misalign popup borders.

Every framebuffer text path must use the shared grapheme/display-cell writer.


### A-23 — forward Delete can split a user-visible grapheme cluster

`src/app/state.rs::delete_forward` deletes `CharacterOffset(current + 1)` for an empty selection.

The editor core's Left/Right and Backspace paths already use grapheme-aware boundaries, so forward Delete is inconsistent. A base character followed by combining marks can be partially deleted.

Forward Delete must delete to the next grapheme boundary, and selected-range Delete must remain one transaction.

### A-24 — key parsing recognizes mature navigation keys that the editor input path silently ignores

The configured-key parser recognizes Home, End, PageUp, and PageDown, but the ordinary editor input match currently handles only Left/Right/Up/Down among navigation keys.

The fallback match returns success without moving the cursor, so unsupported editor-navigation keys can appear accepted while doing nothing.

The stabilization requirements must make required conventional-editor navigation explicit and tested rather than relying on a catch-all no-op.

### A-25 — indentation settings are bypassed by hard-coded formatting/navigation values

The root editor input sends hard-coded tab width `4` to vertical movement instead of the active document `tab_width`.

One LSP document-formatting path also sends `{"tabSize": 4, "insertSpaces": true}` even though `AppState` stores effective `tab_width` and `insert_spaces`.

Result: cursor display-column behavior and formatting can disagree with `.editorconfig` / workspace settings.

All editor movement, indentation, formatting, and display-column conversions must use the same effective document indentation settings.

### A-26 — inlay hints are assigned to a fixed document ID

`language_result_from_response` receives the actual `document: DocumentId`, but the inlay-hint projection constructs every `InlayHintView` with `DocumentId(1)`.

Result: inlay hints can be attached to the wrong tab/pane/document after more than one document identity exists.

Every versioned language result that is document-scoped must preserve the requesting document identity.

### A-27 — contextual language interaction still leaks through generic bottom-panel state

The current language-action path sets `BottomPanelView::Language` and opens the bottom panel for completion acceptance/details, rename, code actions, and related contextual actions.

The application also has contextual-overlay rendering paths, so two competing presentation models remain.

Completion, hover, signature help, Quick Fix/code actions, rename input, and multi-target navigation must use one contextual-overlay interaction state. Problems and Output remain bottom-panel concepts. No contextual action may require toggling the generic Language bottom panel merely to be usable.

## Reference behavior used by the continuation requirements

Microsoft Edit updates its stored TUI size directly when a resize input arrives, and its pointer targeting walks the previously rendered UI tree rather than reconstructing a guessed layout.

Microsoft Edit also provides a persistent menu bar and supports keyboard focusing of that menu.

VS Code preview editors are replaceable until they are kept open; single-click Explorer navigation can open a preview, while modification or non-preview opening keeps the editor open. VS Code also distinguishes preview and pinned/open editor states.

## Scope decision

Requirements 42–51 address the defects above and add automated regression coverage.

They intentionally do not add:

- integrated terminal,
- debugger,
- AI,
- remote development,
- WebView extensions,
- installer signing,
- remote Git host integration.

No requirement in this continuation depends on manual user interaction for acceptance.
