# 045 — live status bar

Owner: WORKBENCH worker

Writable paths:
- `crates/app-ui/src/shell/status.rs`
- matching docs/tests

## Objective

Keep a readable one-row status bar attached to the current terminal bottom and focused document.

## Required implementation

1. Status is one row in normal layout.
2. It is placed from actual current layout height, not cached screen row.
3. Render file/dirty, branch, language, encoding, EOL, indentation, cursor line/column, selection summary, diagnostic counts, LSP, trust as space permits.
4. Compact by dropping lower-priority fields from the right; do not use bullet separators.
5. Use ASCII ` | ` separators.
6. All truncation uses display-cell metrics and ASCII `...`.
7. Background/foreground use dedicated status roles.

## Required tests

- `R003_STATUS_BOTTOM_ROW`
- `U015_STATUS_LIVE`
- compact widths
- CJK filename width

## Done only when

status follows every resize and focused document/cursor update without overlapping other regions.
