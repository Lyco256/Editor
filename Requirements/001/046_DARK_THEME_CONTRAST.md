# 046 — dark theme contrast

Owner: WORKBENCH worker

Writable paths:
- `crates/app-ui/src/shell/theme.rs`
- theme-related UI tests/docs

## Objective

Make the default theme readable on a black editor background and expose distinct semantic roles for chrome.

## Required implementation

1. EditorBackground is exactly `#000000`.
2. Define separate foreground/background pairs for editor text, selection, menu, selected menu, Explorer, selected Explorer, directories, tabs, active tabs, preview, pinned, panel, input, status, separators.
3. True Color normal text pairs have WCAG-style modeled contrast >=4.5:1.
4. 256-color quantization chooses a safe pair if independent nearest colors collide/lose contrast.
5. 16-color mapping guarantees foreground/background ANSI indices differ; critical text uses light/bright foreground on black/dark backgrounds.
6. Selection foreground remains readable on selection background.
7. Active/inactive/preview/pinned remain distinguishable in reduced color.

## Required tests

- `U013_BLACK_EDITOR_BACKGROUND`
- `U014_CONTRAST`
- palette pair tests for True Color/256/16

## Done only when

default dark workbench contains no required text pair that resolves to identical/unreadable foreground/background.
