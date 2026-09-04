# Subagent requirement — workspace UI

## Branch

`feat/ui-workspace`

## Wave

Wave 2. Start only from the tested Wave 1 `devenv` commit.

## Writable ownership

- `crates/app-ui/src/workspace/**`
- corresponding `docs/crates/app-ui/src/workspace/**`
- `tests/fixtures/app-ui/workspace/**`
- `tests/snapshots/ui-workspace/**`

Do not edit shared UI shell files, root application files, other UI feature directories, or `Cargo.lock`.

## Scope

Implement view/action layers for:

- Explorer,
- multi-root workspace display,
- create/rename/move/delete prompts,
- Quick Open,
- project search,
- project replace preview,
- search result navigation,
- recent workspace presentation,
- Workspace Trust prompt and status interaction.

Explorer is visible by default.

Explorer traversal is driven by workspace model events and does not synchronously enumerate the filesystem during drawing.

## Quick Open

Quick Open supports:

- incremental fuzzy filtering,
- keyboard selection,
- mouse selection,
- recent/active file prioritization,
- path display,
- open-in-current-editor action,
- open-to-side action.

## Search UI

Search supports all backend-required options and streams results.

Search cancellation is visible and a newer search supersedes the old result stream.

Multi-file replace shows a preview/count before destructive writes.

## Workspace Trust

An untrusted workspace displays a persistent trust indicator and a clear trust action.

The UI does not bypass the root external-process policy.

## Tests

Framebuffer/state tests cover:

- Explorer tree expansion,
- multi-root rendering,
- compact collapse,
- file operation confirmations,
- Quick Open filtering,
- streamed search results,
- superseded search,
- replace preview,
- untrusted/trusted workspace states.

## Acceptance criteria

- No filesystem I/O occurs in draw functions.
- The UI works from fake workspace models.
- `cargo test -p app-ui` passes for workspace UI cases.
- Clippy passes.
- All owned source files have mirrored documentation.
