# Top Codex — automated MVP gap acceptance

## Purpose

This is the final automated gate for the MVP gap-closure cycle.

It validates only requirements that can be proven from repository-local automation. It does not depend on external service accounts, remote repository administration, installer/signing workflows, or a human-driven interactive test.

## 1. Full repository gates

All must pass:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
- source/document mirror verification
- the repository's authoritative verification scripts that are executable in the current environment

No required test is ignored to obtain a green result.

## 2. Authoritative-state regression gate

Automated tests prove:

- editor frames use persistent tab buffers,
- multiple selections are visible after a frame,
- cursor/status line and column change correctly,
- vertical and horizontal scroll survive frame reconstruction,
- fold state survives frame reconstruction,
- active/focused pane determines status data.

A regression to `TextBuffer::new(active_text)` or frame-local default viewport state must fail a test.

## 3. Editor visual gate

Automated framebuffer tests prove:

- diagnostic text decoration,
- diagnostic gutter markers,
- Git gutter markers,
- whole-document overview ruler,
- diagnostic/search/Git marker priority,
- overview mapping near document start/middle/end,
- multiple cursors,
- inline hints,
- Unicode/wide-character correctness.

## 4. User reachability gate

The command audit proves user reachability for:

- new/open/save/save-as/close,
- tab switching,
- multi-cursor commands,
- splits and pane focus,
- project search,
- Replace in Files,
- encoding reopen/save,
- EOL change,
- Open Recent,
- LSP completion/hover/signature/rename/code action,
- Git high-level and mutation operations.

No required operation may be counted as implemented solely because a backend method or enum variant exists.

## 5. Workspace/search gate

Automated tests prove:

- regex/literal,
- case sensitivity,
- whole word,
- include filter,
- exclude filter,
- streamed/cancelled search,
- replacement-plan preview,
- confirmation,
- replacement execution,
- partial-failure reporting,
- recent-workspace persistence and bounded ordering.

## 6. Language UX gate

Using the fake LSP server, tests prove:

- completion is rendered as a contextual overlay and the selected actual item is applied,
- hover is contextual,
- signature help is contextual,
- code actions are selectable and applied,
- rename input/preview is usable,
- multi-target navigation is selectable,
- inlay hints render inline without changing source text,
- stale overlays/hints are rejected on document version change,
- Problems remains functional as a bottom panel.

## 7. Split gate

Automated tests prove:

- vertical split,
- horizontal split,
- pane focus by keyboard,
- pane focus by mouse hit,
- two different documents render correct independent syntax/status data,
- same document in two panes shares buffer edits but keeps independent scroll/fold state,
- compact layout preserves one interactive focused editor.

## 8. Git gate

Using temporary local Git repositories:

- file stage/unstage through UI routing,
- hunk stage/unstage through UI routing,
- discard confirm/cancel,
- commit/amend,
- branch create/switch/delete confirmation,
- stash create/apply/pop,
- history selection,
- conflict-file action,
- trust-disabled interaction.

Fetch/pull/push routing is tested through typed request construction/fake boundaries only. No network remote is contacted.

## 9. Data-safety gate

Regression tests prove:

- dirty close cannot silently lose text,
- dirty reopen-with-encoding cannot silently lose text,
- failed Save As preserves recoverable text,
- project replace does not write before preview confirmation,
- Git discard does not execute before confirmation,
- stale LSP edits are not applied to a newer document version.

## 10. Documentation gate

Every modified production Rust source file has its mirrored Markdown documentation.

Update `docs/testing/requirements-audit.md` with a new automated gap-closure section containing:

- implemented gap IDs,
- commands used for automated verification,
- passing result summary,
- any remaining item that is outside this continuation scope.

Do not mark an unverified feature as complete.

## Completion criteria

The continuation cycle is complete only when:

- requirements 32 through 40 are integrated on `devenv`,
- every gate above passes,
- the worktree is clean,
- there are no production placeholders introduced by the continuation,
- there are no known original-MVP gaps covered by these continuation documents that remain reachable only through internal APIs.
