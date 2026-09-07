# 050 — contextual LSP overlay screen mapping

Owner: top Codex

Writable paths:
- `src/app/scene.rs`
- `src/app/state.rs` / contextual state routing
- related docs/tests

## Objective

Anchor completion/hover/signature/Quick Fix/rename overlays to the actual pane/caret screen position and eliminate the generic Language-panel dependency.

## Required implementation

1. Identify owning document ID/version/pane for every contextual result.
2. Reject stale/wrong-document result before drawing/applying.
3. Convert document logical position -> display column using effective tab width -> subtract pane viewport -> add actual text-subrect screen origin from layout snapshot.
4. Place overlay below/right; flip/clamp within frame.
5. If anchor is offscreen, dismiss ephemeral overlay.
6. Recompute after scroll, Explorer toggle, split/layout change, and resize.
7. Do not derive global x/y directly from `LogicalPosition.character/line`.
8. Completion, hover, signature, code actions, rename and contextual location chooser no longer require `BottomPanelView::Language`.
9. If generic Language bottom-panel variant has no remaining valid use, remove it.
10. Overlay writing uses shared grapheme/display-cell writer.

## Required tests

- `P008_LSP_OVERLAY_CARET_ANCHOR`
- `P009_LSP_OFFSCREEN_DISMISS`
- `P010_LSP_DOCUMENT_IDENTITY`
- `P011_CONTEXTUAL_NOT_LANGUAGE_PANEL`
- `Q001` overlay part

## Done only when

contextual language UI follows the true caret/pane geometry across scroll/split/resize and cannot leak between documents.
