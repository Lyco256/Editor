# Subagent requirement — UI shell and editor viewport

## Branch

`feat/ui-shell-editor`

## Wave

Wave 2. Start only from the tested Wave 1 `devenv` commit.

## Writable ownership

- `crates/app-ui/src/widgets/**`
- `crates/app-ui/src/shell/**`
- `crates/app-ui/src/editor/**`
- corresponding `docs/crates/app-ui/src/widgets/**`
- corresponding `docs/crates/app-ui/src/shell/**`
- corresponding `docs/crates/app-ui/src/editor/**`
- `tests/fixtures/app-ui/shell-editor/**`
- branch-specific snapshots under `tests/snapshots/ui-shell-editor/**`

Do not edit `app-ui/src/lib.rs`, root application files, other UI feature directories, shared types, or `Cargo.lock`.

## Scope

Implement the common terminal UI shell and editor viewport.

Required shell behavior:

- persistent Explorer allocation point on the left,
- editor area,
- tab strip,
- horizontal and vertical split layout,
- bottom-panel allocation point,
- status bar,
- Command Palette,
- resize/collapse rules,
- focus movement,
- keyboard dispatch,
- mouse focus/click/drag for shell controls.

Required editor viewport behavior:

- line numbers on by default,
- gutter,
- current line state,
- selections,
- multiple cursors,
- text rendering,
- scroll,
- horizontal scroll,
- code folding presentation,
- bracket-match presentation,
- search-match presentation,
- overview ruler,
- dirty tab indicator,
- mouse text selection,
- status data for cursor/selection/encoding/EOL/indent/language.

The overview ruler accepts typed marker sets from language, Git, and search features without depending on their concrete services.

## Command Palette

The palette:

- filters registered commands incrementally,
- supports keyboard-only selection,
- supports mouse selection,
- shows disabled/unavailable commands distinctly,
- never executes commands during filtering.

## Terminal size

When space is insufficient:

1. hide/collapse the bottom panel,
2. collapse the Explorer allocation,
3. reduce nonessential status/tab decoration,
4. preserve at least one interactive editor viewport while dimensions still permit text entry.

## Tests

Use framebuffer snapshots.

Required snapshot scenarios:

- normal 120x40 layout,
- compact 80x24 layout,
- very narrow layout,
- split editors,
- multiple tabs and dirty tab,
- multiple cursors,
- line/gutter/overview markers,
- wide Unicode characters,
- command palette,
- true-color and reduced-color semantic output.

No snapshot depends on actual terminal escape bytes; terminal-backend tests cover that layer.

## Acceptance criteria

- UI reads models and emits actions only.
- UI code performs no filesystem/process/LSP/Git I/O.
- Snapshot tests are deterministic.
- `cargo test -p app-ui` passes for owned test targets.
- Clippy passes.
- All owned source files have mirrored documentation.
