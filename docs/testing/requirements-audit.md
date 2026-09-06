# Requirements audit (2026-09-06)

`devenv` is the current integration branch (the verified state is the tip of `devenv`); it is not yet promoted to `main`
because release gates remain. Automated quality gates pass on `devenv`: formatting, workspace Clippy with warnings denied,
all workspace tests, source/document mirrors, release build, and redirected headless startup.

Implemented in the integration pass:

- persistent root `TextBuffer` editing, smart input, undo/redo, dirty-quit protection, atomic save
  effects, and deterministic save/reopen tests;
- application-data session snapshot/restore for all open tabs, active-tab selection, split layout,
  tab order, multiple selections, missing-file recovery, and unsaved contents;
- root clipboard copy/cut/paste effects (with cut-after-write safety), an authorized external
  formatter effect, and recovery checkpoints after each action;
- canonical-path trust-store loading and saving at process bootstrap;
- multi-root trust decisions now require and persist an explicit state for every canonical root;
- adding an untrusted root revokes aggregate trust and stops any running language server;
- bounded search result backpressure, native `notify` watcher abstraction, typed replacement-range
  validation, typed LSP serialization errors, and panic terminal cleanup;
- root command palette plus Explorer/Output toggles;
- root workspace-root/Explorer projections with asynchronous refresh effects, mouse selection, PATH
  LSP discovery state, and asynchronous Git status effects;
- root syntax refresh/folding/error markers, streaming project-search events with cancellation,
  structured trust-gated LSP request effects with negotiated position encoding, and Git dashboard
  population/status refresh after successful mutations; Tokio scheduling for root background work;
- root command-palette routing for Git Changes, Diff, Branches, Stashes, History, Commit, and
  Conflicts views, plus document-scoped `.editorconfig` indentation, encoding, EOL, whitespace,
  and final-newline behavior;
- confirmed root filesystem-operation prompts and background create/rename/move/delete effects,
  including dirty-buffer deletion protection and Explorer refresh;
- direct terminal paste now participates in format-on-paste precedence and remains undoable;
- root `FindInDocument`/`ReplaceInDocument` actions now expose in-file search and undoable
  replace-all semantics through editor-core, with keyboard prompts and typed invalid-expression
  errors;
- root keyboard prompts now provide Quick Open, project search, and find/replace entry points;
  resolved VS Code keybindings dispatch through the same command path;
- persistent LSP `didChange` notifications now send the smallest changed range derivable from the
  previous/current snapshots;
- workspace-local and static-extension language configuration bracket pairs are applied to smart
  editing, while configuration failures remain visible as compatibility output;
- root headless acceptance coverage for create, rename, move, and delete confirmation workflows;
- Windows installer (`build/editor.iss`) and packaging script (`build/package.ps1`).
- canonical percent-encoded `file:///` URIs for LSP paths containing spaces or non-ASCII text;
- recursive directory WorkspaceEdit rename/delete with rollback journaling;
- runtime VSIX archive discovery for static themes, snippets, and language configuration;
- language-configured block-comment toggling through the command palette and Ctrl+Shift+/.

Release-gate status and evidence:

1. `docs/testing/performance.md` now records release size, idle working-set/private memory, idle CPU,
   repeated redirected startup/close measurements, a deterministic five-iteration redirected 10 MiB
   open/close sample (337.32 ms average, 35.10 MiB average peak working set), a process-level
   five-iteration Windows Terminal 10 MiB open/force-close sample, and a 21.22 ms deterministic
   10 MiB edit transaction. Native keystroke-to-frame latency and clean interactive open/close
   memory behavior remain unavailable; `docs/testing/final-smoke.md` records the PTY smoke scope.
   The master requirement requires user approval before treating those missing measurements as an
   exception.
2. Root action coverage now includes every language-panel action and workspace file-operation
   confirmation path. The headless acceptance suite covers the fake-server initialize/request/response
   lifecycle, semantic tokens, Git projection/mutation routing, encoding round trips, and
   large-file suppression, as well as language panels, cancellation, crash notification, and
   replacement-server startup. A Syntax effect now reaches a rendered Root framebuffer. The
   `.editorconfig` root integration also has deterministic load and save-normalization tests.

3. Server-originated `workspace/applyEdit` requests now pass through root validation, a background
   atomic-save worker, open-tab refresh, and a typed JSON-RPC response. Multi-document edits,
   dirty-buffer rejection, and the fake-server response path have deterministic tests.
4. Root language commands now expose completion resolve, prepare rename, declaration/implementation
   navigation, range formatting, and syntax-aware structural selection expansion.

5. Trusted language-server requests now reuse the persistent client when available, and root queues
   didOpen/didChange/didSave/didClose plus workspace-folder notifications. File resource
   WorkspaceEdit operations (create/rename/delete) are validated, journaled, and applied with
   rollback on operation failure, including recursive directory rename/delete.

6. Client-originated rename/code-action WorkspaceEdits now use the background atomic worker when
   they target unopened documents. Directory rename/delete operations are handled with recursive
   rollback journaling; non-recursive directory deletes return a typed error.

7. Revoking Workspace Trust now stops persistent language-server sessions and rejects late server
   workspace edits, with deterministic root-state coverage.

Static VSIX archive discovery is complete: runtime startup now scans both unpacked extension
directories and `.vsix` archives, extracts archives through the
path-safe compatibility boundary into an OS cache, and loads supported themes/snippets/language
configuration without executing extension code.
Explorer directory rows now expand/collapse through a background refresh, and Ctrl+Shift+E enables
keyboard focus with Up/Down navigation, Left/Right expansion, Enter activation, and Escape return
to the editor. The workspace UI requirement is satisfied by model-driven tree rendering, Quick Open
keyboard/mouse selection, and root file-operation commands; context-menu behavior is outside the initial release scope. Language
configuration now drives line/block comments, word selection, bracket/surrounding pairs,
indentation regexes, and safe on-enter append/remove actions. Folding is syntax-driven as required;
VS Code folding-marker-specific overrides and complex `when` expressions are intentionally not part
of the initial static compatibility contribution set.
Directory resource operations now support recursive rename/delete with in-memory rollback
journaling; non-recursive directory deletes return a typed error. The remaining release gates are
the documented native Windows Terminal/performance evidence (or explicit user-approved platform
limitation) and promotion of this exact verified state to `main`.

## MVP gap-closure audit (Requirements 31–41)

Automated verification rerun on `devenv` (current working commit): `cargo fmt --all`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`,
`cargo test --workspace --test doc_mirror`, and `tools/verify.ps1` all passed; the worktree is
clean and no production `TODO`, `todo!`, or `unimplemented!` remains.

Implemented and covered: persistent tab-buffer rendering, pane ids with viewport/fold/focus state,
cursor/selection status projection, Unicode-safe cursor composition and occurrence selection,
diagnostic inline styling and whole-document overview mode, include/exclude search options, recent
workspace row model (bounded to 20), lifecycle command routes, contextual overlay data contracts,
generic picker/command-registry contracts, pane-aware mouse hit routing, Git keyboard/mouse
dispatch through the trust-gated root adapter, interactive Open Folder and EOL actions, contextual
popup rendering, same-document split-buffer projection, and a dedicated `mvp_gap_acceptance` test.

All audited MVP paths are now covered: the palette materializes the typed registry consumed by
keybinding dispatch, inlay hints retain logical positions and render inline, recent roots persist
through `SessionState`, and the verified `devenv` state has been promoted to `main`.

## Requirements 42–51 and latest UI audit

The Requirements 43–50 implementation is integrated in the root runtime. Rendering and hit testing
consume one `WorkbenchLayoutSnapshot`; resize, split geometry, editor pointer mapping, native
steady-bar cursor presentation, grapheme-safe editing, Explorer/tab/menu state, precise search/Git
ranges, and contextual language placement are covered by deterministic tests. The following matrix
is the release evidence for every finding in `Requirements/LATEST_UI_BUG_AUDIT.md`.

| Finding | Fixing requirement | Automated proof | Status |
|---|---|---|---|
| A-01 | 43 | `shell::tests::resize_matrix_keeps_snapshot_regions_in_frame` | PASS |
| A-02 | 43 | `shell::tests::authoritative_snapshot_hit_testing_uses_current_geometry` | PASS |
| A-03 | 43 | `shell::tests::authoritative_snapshot_hit_testing_uses_current_geometry` | PASS |
| A-04 | 43 | `shell::tests::shell_dispatches_palette_and_mouse_actions` | PASS |
| A-05 | 46, 48 | `editor::tests::viewport_snapshot_handles_line_numbers_folds_unicode_and_markers` | PASS |
| A-06 | 46 | `editor::tests::viewport_snapshot_handles_line_numbers_folds_unicode_and_markers` | PASS |
| A-07 | 47 | `buffer::tests::vertical_navigation_retains_preferred_display_column_across_blank_lines`; `buffer::tests::vertical_navigation_keeps_each_multi_cursor_column` | PASS |
| A-08 | 48 | `shell::tests::chrome_policy_and_grapheme_metrics_preserve_data_cells` | PASS |
| A-09 | 45 | `shell::tests::shell_snapshot_covers_tabs_panels_and_palette` | PASS |
| A-10 | 45 | `shell::tests::variable_width_tab_hit_testing_and_menu_labels_are_exact` | PASS |
| A-11 | 44 | `shell::tests::variable_width_tab_hit_testing_and_menu_labels_are_exact` | PASS |
| A-12 | 49 | `shell::tests::resize_matrix_keeps_snapshot_regions_in_frame` | PASS |
| A-13 | 48 | `widgets::tests::frame_snapshot_records_semantic_styles` | PASS |
| A-14 | 46 | `app::state::tests::mouse_click_and_drag_create_a_logical_selection` | PASS |
| A-15 | 45 | `shell::tests::variable_width_tab_hit_testing_and_menu_labels_are_exact` | PASS |
| A-16 | 50 | `app::state::tests::git_hunk_and_confirmed_discard_dispatch_typed_effects` | PASS |
| A-17 | 48 | `terminal::tests::cleanup_state_is_ordered_and_idempotent` | PASS |
| A-18 | 43 | `shell::tests::authoritative_snapshot_hit_testing_uses_current_geometry` | PASS |
| A-19 | 50 | `app::runtime::tests::root_frame_projects_syntax_roles_and_folds` | PASS |
| A-20 | 50 | `app::state::tests::workspace_search_events_update_only_the_active_session` | PASS |
| A-21 | 50 | `app::runtime::tests::root_frame_renders_interactive_language_result_panel` | PASS |
| A-22 | 48 | `shell::tests::chrome_policy_and_grapheme_metrics_preserve_data_cells` | PASS |
| A-23 | 47 | `buffer::tests::wide_and_combining_graphemes_move_as_units`; `smart::tests::combining_grapheme_is_deleted_together` | PASS |
| A-24 | 47 | `input::tests::key_normalization_preserves_modifiers_and_repeat` | PASS |
| A-25 | 47 | `app::state::tests::workspace_settings_enable_formatting_and_configure_viewport` | PASS |
| A-26 | 50 | `app::state::tests::lsp_results_update_versioned_language_views` | PASS |
| A-27 | 50 | `app::state::tests::contextual_language_actions_keep_bottom_panel_selection_stable` | PASS |

Repository gates are run on the same integration commit with formatting, warnings-denied Clippy,
all-feature workspace tests, and the source/document mirror test. This document intentionally does
not claim `main` promotion or a remote push until that exact clean, verified `devenv` commit is
created and pushed.
