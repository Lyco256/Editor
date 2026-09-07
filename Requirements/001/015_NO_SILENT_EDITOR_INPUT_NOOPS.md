# 015 — no silent editor input noops

Owner: top Codex

Writable paths:
- `src/app/editor_input.rs`
- `src/app/state.rs`
- command/keybinding routing docs

## Objective

Prevent recognized conventional editor keys from silently returning success without behavior.

## Required implementation

1. Replace the broad editor-key `_ => Ok(())` completion behavior with explicit classification: handled editor key, command-routed key, or genuinely unsupported input.
2. Home, End, PageUp, PageDown, modifier navigation, Delete, Tab/ShiftTab, and the required line operations must route to explicit editor-core operations after lane CORE is merged; until then they return an explicit unimplemented-route test state, not silent success.
3. Normal character insertion must not intercept Ctrl/Alt/Meta command chords.
4. Add a registry/test table for required editor key routes so every required combination maps to a command/operation.
5. Unknown keys may be ignored, but they are not listed as supported commands.

## Required tests

- `K023_NO_SILENT_NAV_NOOP`
- key-route table completeness test

## Done only when

every required key has an explicit executable route and no supported navigation key falls through a generic successful no-op.
