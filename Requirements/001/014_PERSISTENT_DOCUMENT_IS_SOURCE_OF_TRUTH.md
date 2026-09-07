# 014 — persistent document is source of truth

Owner: top Codex

Writable paths:
- `src/app/scene.rs`
- `src/app/state.rs`
- matching docs

## Objective

Eliminate the parallel rendered-document fallback through `active_text`; persistent tab `TextBuffer` is authoritative.

## Required implementation

1. Scene projection obtains the displayed document from the pane's tab/document identity.
2. It uses that persistent `TextBuffer` snapshot and selections directly.
3. Remove the conditional branch that replaces it with `TextSnapshot::from_text(state.active_text.clone())`.
4. `active_text`, if retained for legacy serialization compatibility, is write-only projection data and never a rendering/editing/navigation source.
5. Status dirty/cursor/selection/encoding/EOL values come from the focused displayed document.
6. Add a debug/test invariant that compatibility projection may differ without changing rendered authoritative text.

## Required tests

- `P001_ACTIVE_BUFFER_AUTHORITATIVE`
- cursor/selection status updates from persistent buffer
- stale `active_text` regression fixture

## Done only when

rendering, editing, selection, status, LSP versioning, and pane projection all identify the persistent document model, not a duplicate string.
