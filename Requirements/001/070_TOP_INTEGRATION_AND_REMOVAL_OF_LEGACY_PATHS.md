# 070 — top integration and removal of legacy paths

Owner: top Codex

Writable paths:
- all top-owned integration paths
- docs/testing evidence

## Objective

Wire merged lanes into root runtime and delete every obsolete competing implementation.

## Required implementation

1. Route all required key commands to CORE operations using effective `tab_width`/indent settings.
2. Route pointer through stored layout snapshot into POINTER actions.
3. Present primary caret via TERMINAL cursor presentation.
4. Build scene with WORKBENCH components.
5. Implement Preview/Open/Pinned root state machine:
   - single Explorer click -> Preview;
   - one clean Preview per group;
   - next Preview replaces it;
   - edit Preview -> Open before replacement;
   - double click/Enter -> Open;
   - Pin -> Pinned; Unpin -> Open;
   - pinned survives Close Others.
6. Keep each editor group local tabs/pane state.
7. Apply minimal cursor reveal after keyboard/editing, not after wheel only.
8. Remove old fixed coordinate code.
9. Remove old `▌` cursor rendering path.
10. Remove raw `.find(matched_text)` project-marker path.
11. Remove whole-file ordinary Git marker path.
12. Remove direct document-line/character-as-screen-coordinate LSP overlay path.
13. Remove generic Language bottom-panel dependency if no valid remaining use.
14. Use effective tab width everywhere; remove navigation literal 4.
15. Root forward Delete uses core grapheme delete.
16. Ensure actual resize is runtime-owned and immediately rendered.

## Required tests

Run frozen acceptance suite, 060–064 integration suites, full workspace tests, format, Clippy, docs mirror.

## Done only when

there is one implementation path for each required behavior, all frozen cases pass, and no old fallback can be selected at runtime.
