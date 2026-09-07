# Subagent requirement — theme contrast and terminal presentation

## Branch

`feat/theme-terminal-presentation`

## Writable ownership

- `crates/terminal-backend/src/capability.rs`
- `crates/terminal-backend/src/renderer.rs`
- `crates/terminal-backend/src/terminal.rs`
- `crates/terminal-backend/src/input.rs`
- matching mirrored docs
- terminal-backend-specific fixtures/tests

Do not edit terminal-backend public facade if Stage 43 marked it top-owned, root `src/app/**`, app-ui, editor-types, or `Cargo.lock`.

## Goal

Make the default dark UI readable on a black editor background at True Color, 256-color, and 16-color capability levels, and implement the generic caret/click contracts frozen by Stage 43.

## 1. Built-in dark palette

The built-in default theme uses these True Color base values:

| Role | RGB |
|---|---|
| EditorBackground | `#000000` |
| EditorText | `#D4D4D4` |
| CurrentLineBackground | `#101010` |
| SelectionForeground | `#FFFFFF` |
| SelectionBackground | `#264F78` |
| SecondaryCursorForeground | `#FFFFFF` |
| SecondaryCursorBackground | `#5A5A00` |
| LineNumber | `#858585` |
| LineNumberActive | `#C6C6C6` |
| Gutter | `#858585` |
| SplitSeparator | `#4A4A4A` |
| MenuForeground | `#F0F0F0` |
| MenuBackground | `#1E1E1E` |
| MenuSelectedForeground | `#FFFFFF` |
| MenuSelectedBackground | `#094771` |
| ExplorerForeground | `#D4D4D4` |
| ExplorerBackground | `#0C0C0C` |
| ExplorerDirectory | `#4FC1FF` |
| ExplorerSelectedForeground | `#FFFFFF` |
| ExplorerSelectedBackground | `#264F78` |
| TabForeground | `#C8C8C8` |
| TabBackground | `#111111` |
| TabActiveForeground | `#FFFFFF` |
| TabActiveBackground | `#000000` |
| TabPreviewForeground | `#A9D4FF` |
| TabPinnedForeground | `#DCDCAA` |
| PanelForeground | `#D4D4D4` |
| PanelBackground | `#0A0A0A` |
| PanelTitle | `#FFFFFF` |
| InputForeground | `#FFFFFF` |
| InputBackground | `#1E1E1E` |
| StatusForeground | `#FFFFFF` |
| StatusBackground | `#007ACC` |
| Border | `#5A5A5A` |

Existing diagnostic/Git/syntax colors may be retained only when they satisfy the contrast rules below against the background on which they are used.

## 2. True Color contrast rules

For normal-sized text, the modeled contrast ratio between foreground and background is at least 4.5:1 for:

- editor text,
- selected text,
- menu text,
- selected menu text,
- Explorer file/directory text,
- selected Explorer text,
- tab text,
- active tab text,
- preview/pinned tab text,
- panel text,
- input text,
- status text.

Add deterministic contrast-ratio tests.

## 3. 256-color fallback

Resolve foreground/background as a pair.

For required readable text pairs:

- resolved foreground and background palette indices must differ,
- modeled xterm-palette contrast remains >= 4.5:1.

If independent nearest-color quantization would violate the rule, choose the nearest safe fallback foreground/background pair.

## 4. 16-color fallback

Because terminal users may customize ANSI palette RGB values, exact real-world contrast cannot be guaranteed.

The implementation must still ensure:

- required foreground/background roles resolve to different ANSI indices,
- EditorBackground resolves to black index 0 in the built-in theme,
- critical normal text on black uses light/bright foreground indices,
- selected/menu/status text never resolves to the same index as its background.

Tests use the standard ANSI palette model and require >= 4.5:1 where a safe pair exists.

## 5. Pair-aware rendering

Do not assume that because two semantic roles have different names they necessarily resolve to readable colors.

Renderer style resolution sees both foreground and background and can apply the safe fallback policy before output.

## 6. Terminal cursor presentation

Implement the Stage 43 generic terminal cursor contract using the existing crossterm capabilities.

For a visible editor caret:

1. render framebuffer diff,
2. move terminal cursor to the requested screen cell,
3. select `SteadyBar`,
4. show cursor,
5. flush.

For hidden cursor:

- hide cursor after frame presentation.

If the requested caret cell is outside the current framebuffer, hide the cursor instead of emitting an invalid position.

Terminal restoration returns cursor shape/visibility to a safe default.

## 7. Resize presentation

The existing differential renderer's frame-size change path performs a full redraw.

Add regression tests proving that:

- a frame size change triggers full redraw,
- the old-size framebuffer is not indexed using the new-size dimensions,
- shrinking clears stale content outside the new workbench by the terminal clear/full-redraw sequence,
- expanding paints the complete new frame.

## 8. Click-count input

Implement the Stage 43 multi-click contract in the stateful input reader/normalization layer.

Tests use an injectable deterministic clock and cover:

- single click,
- double click,
- triple click,
- timeout reset,
- coordinate change reset,
- different-button reset,
- drag reset.

Do not make tests sleep in real time.

## Tests

In addition to the above:

- every required role exists in the default palette,
- default EditorBackground is exactly `#000000`,
- all required True Color pairs meet contrast,
- 256-color pairs meet contrast after quantization/fallback,
- 16-color critical pairs are never equal,
- SteadyBar cursor commands are emitted,
- hidden cursor commands are emitted for non-editor focus,
- out-of-frame cursor is hidden,
- restoration remains idempotent.

## Acceptance criteria

- default dark theme has no known foreground/background equality for required text surfaces,
- editor background is black,
- terminal cursor presentation is available through the generic adapter contract,
- cursor shape is a thin steady bar for normal focused editing,
- resize and multi-click terminal tests pass,
- all owned tests, docs, format, and Clippy checks pass.
