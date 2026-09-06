# Subagent requirement — pane geometry and information density

## Branch

`feat/pane-density`

## Wave

Wave 4B. Start only after Wave 4A is integrated and green.

## Writable ownership

- `crates/app-ui/src/shell/panes.rs`
- matching mirrored docs
- pane/split-specific fixtures and snapshots assigned by the top foundation

Do not edit root `src/app/**`, shell facade/geometry/workbench/menu/explorer/tabs, editor modules, shared types, terminal backend, or `Cargo.lock`.

## Goal

Make each editor group use space like a dense VS Code-style workbench while retaining Edit-like simplicity.

## 1. Editor group geometry

Each editor group consists of:

1. one-row group-local tab strip,
2. editor content rectangle below it.

There is no extra outer box around the group.

The content rectangle is subdivided horizontally into:

- dynamic line-number region,
- compact gutter/fold/marker region,
- text region,
- one-cell overview ruler.

No unexplained right-side region is permitted.

## 2. Split separators

Vertical split:

- exactly one separator column.

Horizontal split:

- exactly one separator row.

Separator style uses `SplitSeparator`.

Do not draw a full rectangle border around both children before drawing the separator.

## 3. Split behavior

Split geometry supports nested pane trees.

The focused group remains visually identifiable using tab/background/separator styling without adding an extra full border.

Ratios clamp so both child groups retain at least:

- one tab row,
- one editor text column after line-number/gutter/overview overhead,
- one editor text row.

When the terminal becomes too small, preserve the focused editor and collapse lower-priority regions according to the established compact policy.

## 4. Bottom panel

The bottom panel is attached below the editor-group area, not below Explorer.

When hidden, its rows are returned entirely to editor groups.

When visible, it has:

- one title/header row if needed,
- content rows,
- at most one top separator row.

Do not reserve empty bottom-panel rows when the panel is hidden.

## 5. Layout coverage invariant

The shell geometry test classifies every screen cell in the normal workbench as one of:

- menu,
- Explorer,
- editor-group tab,
- line number,
- gutter,
- editor text,
- overview,
- split separator,
- bottom panel,
- status,
- overlay.

There is no persistent “unknown/filler” layout region.

Blank cells inside editor text are valid editor content and do not count as filler.

## 6. No phantom split

If the application has one editor group, render one group using the complete editor area.

Do not render an empty second group or a horizontal-line placeholder merely because split infrastructure exists.

A second group is created only by an explicit split/open-to-side action or restored session state.

## 7. Pane-local hit geometry

The `WorkbenchLayoutSnapshot` records exact pane-local subrectangles after dynamic line-number/gutter widths are known.

The editor pointer mapper consumes those exact text/gutter/overview rectangles.

## Tests

Snapshot/layout tests cover:

- one group 120x40,
- one group 80x24,
- vertical split,
- horizontal split,
- nested split,
- hidden bottom panel gives rows back,
- visible bottom panel,
- Explorer visible/hidden,
- no phantom split,
- overview exactly one column,
- only one-cell split separators,
- no outer pane border,
- layout coverage invariant,
- compact collapse.

## Acceptance criteria

- the unexplained repeated-horizontal-line space is structurally impossible,
- one group uses the full available editor area,
- tabs are group-local,
- no unnecessary pane border consumes cells,
- pointer subrectangles match rendered subrectangles,
- all owned tests, docs, format, and Clippy checks pass.
