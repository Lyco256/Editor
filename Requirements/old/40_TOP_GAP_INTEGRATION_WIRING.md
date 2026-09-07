# Top Codex — MVP gap integration wiring

## Goal

After Wave 3 is merged and green, wire every completed model/view into the authoritative root runtime so the original MVP features are reachable from normal Editor interaction.

The top Codex owns all changes in `src/app/**`, shared types, central registrations, and cross-feature integration tests.

## 1. Core file/editor command surface

Register and route these commands:

- New File
- Open File
- Open Folder / Add Folder to Workspace
- Save
- Save As
- Close Active Editor
- Close Other Editors
- Next Editor Tab
- Previous Editor Tab
- Quick Open
- Open Recent

Dirty Close and dirty Open/Replace flows must protect unsaved data.

Path-taking commands use terminal-native input/pickers. They do not depend on a native GUI file dialog.

## 2. Multi-cursor command surface

Register and route:

- Add Cursor Above
- Add Cursor Below
- Add Cursor at mouse position when the terminal reports the required modifier
- Add Selection to Next Find Match
- Skip Current Selection and Select Next Match
- Select All Occurrences
- Remove Last Secondary Cursor
- Collapse to Single Cursor

The default keybindings follow VS Code conventions where the terminal can reliably distinguish the chord. Where it cannot, the command remains available in the Command Palette and a conflict-free default terminal binding is assigned.

All editing actions use the authoritative multi-selection `TextBuffer`.

## 3. Split pane integration

Use the pane model established by Stage 32 and shell pane identities from requirement 38.

Required behavior:

- Split Editor Right / vertical
- Split Editor Down / horizontal
- Close Focused Split
- Focus Left/Right/Up/Down Group
- mouse focus by pane
- independent scroll positions
- independent collapsed folds
- each pane renders the syntax, search markers, diagnostics, bracket matches, status, and inline hints of the document actually displayed in that pane.

Do not reuse the active primary document's syntax/diagnostic/status projection for another document displayed in a secondary pane.

At least two simultaneously interactive panes are mandatory. Existing nested shell support must not be broken.

## 4. Viewport and status wiring

Project each pane directly from its displayed persistent tab buffer.

Wire:

- selections/cursors,
- viewport scroll,
- folds,
- syntax spans,
- semantic spans,
- diagnostics,
- search matches,
- Git line markers,
- bracket matches,
- inline inlay hints.

The focused pane determines the global status-bar cursor/document fields.

## 5. Diagnostics and Git markers

Build editor marker inputs from real document coordinates.

Diagnostics appear:

- as inline text decoration,
- in gutter,
- in overview ruler,
- in Problems.

Git diff data for the open document is converted into line markers:

- added,
- modified,
- deleted.

These appear in gutter and overview ruler.

Search markers appear in overview ruler for the complete document.

## 6. Contextual language UI routing

Completion, hover, signature help, Quick Fix/code actions, rename, and multi-target navigation use the contextual overlay renderers from requirement 36.

Do not force the bottom panel to "Language" merely to display these cursor-context features.

Problems remains a bottom panel.

Inlay hints are mapped to the editor inline-hint primitive and displayed at their LSP positions without modifying text.

Completion acceptance applies the selected completion item's actual LSP edit/insert text semantics rather than inserting the displayed label blindly.

## 7. Project search and replace routing

Expose a Search workspace interaction that allows changing:

- regex/literal,
- case sensitive,
- whole word,
- include filter,
- exclude filter.

Expose Replace in Files.

Replace in Files must:

1. run the search,
2. build a replacement plan,
3. show preview counts,
4. require confirm,
5. apply the plan,
6. show success/partial failure result.

The replacement operation remains cancellable before confirmation.

## 8. Encoding and EOL commands

Replace UTF-only command proliferation with these user-facing commands:

- Reopen with Encoding
- Save with Encoding
- Change End of Line Sequence

`Reopen with Encoding` and `Save with Encoding` open the generic picker populated from every encoding supported by the existing encoding subsystem, including UTF-8 variants, UTF-16 LE/BE, Shift_JIS-compatible encoding, and all canonical legacy encodings exposed by the current `encoding_rs` integration.

Reopen refuses to discard a dirty buffer without an explicit discard/save resolution.

Change EOL provides:

- LF
- CRLF

Changing encoding or EOL marks the document dirty when it changes the next saved representation.

## 9. Recent workspaces

Persist recent workspace roots in the existing application-data configuration/session area.

Opening a workspace updates recency order.

`Open Recent` uses the generic picker.

Missing paths remain visible but disabled until removed by an explicit Remove Recent action.

Store a bounded maximum of 20 recent entries.

## 10. Git interaction routing

When Source Control is focused, keyboard and mouse events route to the Git interaction model.

Wire all original MVP Git operations:

- status refresh,
- diff,
- stage/unstage file,
- stage/unstage hunk,
- discard with confirmation,
- commit,
- amend,
- branch create/switch/delete,
- fetch,
- pull,
- push,
- stash create/apply/pop/list,
- history,
- conflict-file open.

Also expose Command Palette entries for high-level Git operations that remain useful without panel focus:

- Git: Refresh
- Git: Commit
- Git: Fetch
- Git: Pull
- Git: Push
- Git: Create Branch
- Git: Switch Branch
- Git: Stash
- Git: Show Source Control

Do not add network-dependent automated tests.

## 11. Command reachability invariant

Add an automated command audit.

For every command in the default registry:

- it has a valid action route,
- its availability predicate is testable,
- invoking it in a valid synthetic state either changes state, emits an effect, or opens a typed interaction.

For every original-MVP operation listed in this file, at least one normal user route exists through keyboard, mouse, Command Palette, or a visible focused control.

An internal enum variant alone does not satisfy reachability.

## 12. Tests

Add cross-feature deterministic tests for:

- New/Open/Save As/Close,
- tab next/previous/close,
- Open Recent,
- every generic encoding command path with representative legacy encoding,
- LF/CRLF change,
- multi-cursor creation and multi-edit,
- two independently focused panes,
- secondary pane with different document syntax/status,
- completion contextual overlay and acceptance,
- hover/signature contextual overlay,
- inline inlay hint,
- diagnostic inline/gutter/overview projection,
- Git gutter/overview markers,
- project search options,
- Replace in Files preview/confirm,
- Git panel keyboard stage/unstage and commit routing,
- Command Palette reachability audit.

Tests use fake terminals, fake LSP, temp files, and temp Git repositories. They do not require a human to interact with the process.

## Acceptance criteria

- Every original-MVP capability addressed by requirements 33–39 is reachable through the real root runtime.
- The renderer projects authoritative persistent document/pane state.
- Secondary panes no longer inherit unrelated primary-pane language/search/status data.
- cursor-context language UX is contextual rather than bottom-panel-only.
- project replace is actually executable through UI state after preview.
- full supported encoding selection and EOL selection are user-accessible.
- required Git mutations are user-accessible.
- the command reachability audit passes.
- full workspace format, Clippy, tests, and docs mirror checks pass.
