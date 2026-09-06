# Subagent requirement — Explorer and editor-tab UX

## Branch

`feat/explorer-tabs-ux`

## Writable ownership

- `crates/app-ui/src/shell/explorer.rs`
- `crates/app-ui/src/shell/tabs.rs`
- matching mirrored docs
- Explorer/tab-specific fixtures and snapshots assigned by the top foundation

Do not edit shell facade/geometry/workbench/menu/panes, root `src/app/**`, shared types, editor core, terminal backend, or `Cargo.lock`.

## Goal

Make files, folders, preview editors, normal editors, pinned editors, dirty editors, and active editors visually and behaviorally unambiguous without font-dependent icons.

## 1. Explorer entry model

The presentation model includes an explicit entry kind:

- File
- Directory

It does not infer kind only from `expanded`.

Directory names are rendered with a trailing `/`.

## 2. Explorer structural notation

Use only the approved chrome glyph set.

Default directory prefixes:

- collapsed directory: `[+] `
- expanded directory: `[-] `

Default file prefix:

- `    `

Indentation may additionally use ASCII or Box Drawing tree guides.

Do not use icon fonts, emoji, triangles, bullets, folder glyphs, or Nerd Font symbols.

## 3. Explorer color distinction

Default dark-theme semantic roles are used so:

- directories use `ExplorerDirectory`,
- files use `ExplorerForeground`,
- selected row uses `ExplorerSelectedForeground` + `ExplorerSelectedBackground`,
- focused selected row remains distinguishable from an inactive active-file row.

Color is an additional distinction; trailing `/` and `[+]/[-]` ensure the distinction remains understandable in reduced-color modes.

## 4. Explorer behavior

Required file interaction:

- single left click File -> request Preview open in the focused editor group,
- double left click File -> request permanent Open,
- Enter on selected File -> permanent Open,
- Space on selected File -> Preview,
- Alt+Click File -> open permanent editor to the side when split creation is available.

Required directory interaction:

- single left click directory row selects it,
- click `[+]`/`[-]` zone toggles expansion,
- double click directory row toggles expansion,
- Enter toggles expansion.

Explorer scroll keeps the selected row visible.

Mouse wheel over Explorer scrolls Explorer, not the editor.

## 5. Tab disposition model

The shell presentation distinguishes three dispositions supplied by root state:

- Preview — temporary and replaceable,
- Open — normal permanent editor,
- Pinned — explicitly protected/pinned editor.

Visual ASCII conventions:

- Preview title is wrapped as `<name>`,
- Open title is `name`,
- Pinned title is prefixed with `^`,
- dirty state prefixes `*`,
- close target uses ASCII `x` when visible.

Active/inactive state is also represented by semantic foreground/background roles.

Examples:

- Preview: `<README.md>`
- Active dirty Open: `*main.rs`
- Pinned: `^config.toml`

Do not rely only on italic text, since italic support is not a terminal requirement.

## 6. Preview semantics

Within one editor group there is at most one Preview editor.

Opening another file as Preview replaces the existing clean Preview in that group.

A Preview becomes Open when:

- its document is modified,
- it is double-clicked in Explorer,
- its tab is double-clicked,
- the Keep Open command is executed,
- Enter opens the same Explorer file permanently,
- it is moved/opened to another group as a permanent editor.

A dirty document is never replaceable as Preview.

Opening a file that is already visible in the target group focuses the existing tab instead of duplicating it unless an explicit open-to-side operation requests another view.

## 7. Pinned semantics

Pin Editor changes Open -> Pinned.

Unpin Editor changes Pinned -> Open.

Pinned tabs:

- sort before non-pinned tabs in the same group,
- are not removed by Close Other Editors,
- remain individually closeable by explicit Close Editor.

Preview cannot directly remain Preview while pinned; pinning first converts it to Open and then Pinned.

## 8. Exact tab geometry

Tab rendering produces an exact rectangle for every visible tab and close target in the layout snapshot.

Pointer hit testing uses these rectangles.

No tab index is derived from `column / fixed_width`.

All title truncation uses shared display-cell metrics and ASCII `...`.

Tab overflow uses a scrollable tab strip or deterministic clipping with visible left/right ASCII navigation markers. It must not overwrite adjacent workbench regions.

## Tests

Tests cover:

- File vs Directory notation,
- reduced-color readability,
- single-click Preview,
- double-click Open,
- Enter Open,
- Space Preview,
- preview replacement,
- dirty preview promotion,
- tab double-click promotion,
- Pin/Unpin,
- Close Others preserves pinned,
- variable-width ASCII/CJK file names hit exact tabs,
- long title truncation by display width,
- tab overflow,
- mouse wheel Explorer scroll,
- no decorative forbidden glyph in rendered chrome.

## Acceptance criteria

- file/folder difference is visible without color,
- preview/open/pinned difference is visible without italic support,
- Preview replacement follows the stated state machine,
- tab hit testing uses rendered rectangles,
- layout is grapheme/display-cell safe,
- all owned tests, docs, format, and Clippy checks pass.
