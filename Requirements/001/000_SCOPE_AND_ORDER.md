# 001 — stabilization scope and execution order

## Scope

This cycle repairs the editor/TUI behavior that is still visibly broken after earlier requirement cycles.

The reference hierarchy is fixed:

1. For primitive terminal editor behavior that Microsoft Edit supports, reproduce Microsoft Edit semantics.
2. For conventional code-editor behavior not present in Edit, reproduce the exact behavior stated here, based on VS Code conventions already chosen by the project.
3. Do not replace required behavior with a different UX merely because it is easier to implement.

## Required outcomes

This cycle must make these areas reliable:

- immediate terminal resize and redraw;
- one geometry source shared by rendering and mouse hit testing;
- conventional keyboard cursor/navigation behavior;
- preserved vertical preferred display column;
- grapheme-safe delete/backspace and motion;
- accurate mouse placement, drag, click-count selection, and wheel scroll;
- thin terminal-native primary caret that never destroys the source glyph;
- readable selections and secondary cursors;
- Edit-like top menu;
- dense VS Code-like workbench;
- clear file/folder tree;
- Preview/Open/Pinned tab semantics;
- correct split-pane isolation;
- black-background dark theme with contrast-safe text;
- ASCII/Box-Drawing-only chrome;
- exact project-search and Git hunk markers;
- contextual LSP overlays anchored to the actual pane/caret.

## Execution order

The order is mandatory:

1. 001–005: acceptance oracle setup and freeze.
2. 006: mechanical decomposition only.
3. 010–015: top-owned input/layout foundation.
4. parallel lanes 020–047.
5. merge all green lanes.
6. top-owned 050–053.
7. 060–064 test expansion/checks.
8. 070 integration.
9. 071–074 independent reviews.
10. fix blockers and repeat reviews until zero.
11. 075 verifier.
12. 076 completion decision.

Do not start a later dependency stage while an earlier required stage is red.
