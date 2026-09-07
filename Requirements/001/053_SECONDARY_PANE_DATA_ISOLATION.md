# 053 — secondary pane data isolation

Owner: top Codex

Writable paths:
- `src/app/scene.rs`
- `src/app/state.rs`
- matching docs/tests

## Objective

Project every editor pane from its own document/pane state rather than reusing primary-pane semantic state.

## Required implementation

For each pane independently resolve:
- persistent TextBuffer snapshot/selections;
- viewport/folds;
- syntax spans;
- semantic tokens;
- diagnostics;
- search matches;
- bracket matches;
- Git markers;
- inlay hints;
- file/status metadata.

Do not hard-code secondary `pane_id = 1` as the only secondary identity.
Do not clone primary `language_markers`, `git_markers`, `status`, or inlay view into another document.
Focused pane alone supplies global status bar cursor/document values.

Same document in two panes shares text and semantic document data, but viewport/folds remain pane-local.

## Required tests

- `P003_SECONDARY_PANE_ISOLATION`
- two different documents with deliberately different syntax/diagnostic/search/inlay fixtures
- same document two panes independent scroll/fold

## Done only when

each pane displays only data belonging to its displayed document and pane-local view state.
