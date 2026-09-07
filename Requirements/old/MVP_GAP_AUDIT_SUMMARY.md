# Current MVP gap audit summary

Audit target: `Lyco256/Editor`, `devenv`, reviewed 2026-09-06.

## Confirmed implementation strengths

The current repository already contains substantial backend/domain implementation for:

- persistent Rope-based editing,
- undo/redo and smart editing,
- workspace/recovery/trust,
- Tree-sitter,
- LSP client and fake server,
- Git backend and dashboard models,
- static VS Code compatibility,
- Explorer/search models,
- terminal framebuffer/backend,
- automated test and source-doc mirror infrastructure.

The continuation requirements therefore focus on reachability and correct projection rather than reimplementing these systems.

## Confirmed remaining MVP gaps addressed by Requirements 32–41

1. Root rendering reconstructs a fresh buffer from `active_text`, so authoritative selections/cursor state are not projected reliably.
2. Root rendering constructs default viewport state per frame, so persistent scroll/fold/pane view state is incomplete.
3. Status projection leaves cursor/selection fields at defaults.
4. The secondary split projection reuses primary language/status data and lacks full independent pane interaction.
5. Editor marker logic compares character offsets with line numbers in the overview path.
6. The overview ruler currently follows visible viewport lines instead of proportionally representing the whole document.
7. Diagnostics are present in Problems/marker data but the editor glyph styling path does not apply diagnostic text decoration.
8. Git marker storage exists in `SemanticMarkerSet`, but root projection does not populate Git markers into the editor.
9. Multiple selections exist in core/session state, but normal add-cursor/next-occurrence user commands are missing.
10. Project search UI does not expose the complete option set and Replace in Files is not wired as a complete user flow.
11. Language completion/hover/signature/code action/inlay presentations are routed through a generic bottom Language panel rather than the required contextual/inline UX.
12. The Git dashboard has typed mutation actions but no complete keyboard/mouse interaction route for those actions.
13. Core `Action` variants exist for Save As/tab/split/encoding flows, while the default command surface omits several essential file/editor lifecycle operations.
14. Encoding commands expose only UTF-8/UTF-16 fixed commands despite broader backend encoding support.
15. Recent-workspace state is represented in workspace UI models but is not integrated into root recent-workspace persistence/command flow.

## Not duplicated

These continuation documents intentionally do not request reimplementation of already-present LSP protocol methods, Tree-sitter language parsers, Git command backend operations, VSIX static extraction, Workspace Trust enforcement, recovery, or existing installer files unless required by a gap integration test.
