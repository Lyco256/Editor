# Top Codex — foundation stage B: shared contracts and vertical skeleton

## Goal

Freeze the cross-feature contracts before parallel implementation so feature branches can work independently and merge with minimal conflict.

## Required shared contracts

The top Codex implements in `editor-types` and root `src/app/**`:

- document/workspace identifiers,
- explicit text position/range newtypes,
- normalized keyboard and mouse action types,
- command identifiers,
- diagnostic model and severity,
- theme/style semantic roles,
- Git-status summary types needed by UI,
- language-server status summary types needed by UI,
- application `Action`,
- asynchronous `Effect`,
- asynchronous completion `Event`,
- root `AppState`,
- feature/view registration contracts,
- cancellation/request identifiers,
- output/log message model.

The contracts expose data, not concrete service implementations.

## App loop skeleton

The root application:

- enters the terminal through the terminal adapter,
- starts the unidirectional event loop,
- accepts normalized input,
- updates root state,
- dispatches effects,
- receives events,
- requests a frame,
- renders through a framebuffer abstraction,
- restores terminal state on clean exit and panic cleanup paths.

The skeleton does not implement full editor, syntax, Git, LSP, workspace, or feature UI behavior.

## Headless path

Foundation includes a headless terminal/frame adapter so integration tests can run without a real interactive terminal.

A smoke test starts the root runtime with fake adapters, sends a quit action, receives at least one frame, and exits cleanly.

## Shared contract stability

After Stage B passes, Wave 1 branches treat all shared contracts as frozen.

## Acceptance criteria

Stage B is complete only when:

- the root app starts and exits through fake adapters,
- the event/effect direction is enforced by crate boundaries,
- there is no mutable global application state,
- no feature worker can directly mutate UI state,
- the workspace compiles with every feature crate stub connected,
- panic/exit cleanup is exercised by tests where feasible without terminating the test process,
- full foundation verification passes,
- the exact tested `devenv` commit is used as the base commit for every Wave 1 worktree.
