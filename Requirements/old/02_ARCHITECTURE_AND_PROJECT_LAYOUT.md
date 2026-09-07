# Architecture and project layout

## 1. Architectural pattern

Editor uses a **unidirectional event-driven architecture with ports/adapters boundaries**.

MVVM is not used as the primary application architecture.

The root application owns:

- authoritative application state,
- normalized user actions,
- asynchronous effect requests,
- effect completion events,
- feature registration,
- rendering cadence.

Feature code never directly mutates another feature's state.

The flow is:

`Input -> Action -> State update -> Effect request -> background service -> Event -> State update -> View -> Framebuffer`

Views read state and produce drawing commands. Views do not perform filesystem access, Git commands, language-server requests, or process execution.

Services perform external I/O and send typed results back to the root application.

## 2. Concurrency model

The UI state has one authoritative mutation path.

Tokio is the asynchronous runtime for process I/O, LSP, search orchestration, and background services.

No background task receives mutable access to UI state.

Communication uses bounded channels for high-volume streams where backpressure matters and typed one-shot responses for request/response operations.

Cancellation is required for replaceable work such as completion requests, searches, syntax jobs, and stale workspace scans.

## 3. Cargo workspace structure

The top-level project structure is:

- `src/` — binary/application orchestration only.
- `crates/` — independently testable domain and adapter crates.
- `tests/` — cross-crate integration and end-to-end tests.
- `docs/` — architecture, testing, and source-mirror documentation.
- `Requirements/` — implementation requirements.
- `tools/` — repository verification and developer scripts.
- `build/` — installer and packaging definitions/scripts, not compiled output.
- `assets/` — built-in theme data and other shipped static resources.
- `target/` — Cargo-generated output, ignored by Git.

Required workspace crates:

- `editor-types`
- `editor-core`
- `terminal-backend`
- `config-core`
- `workspace-core`
- `syntax-engine`
- `lsp-client`
- `vcs-git`
- `vscode-compat`
- `app-ui`

## 4. Dependency direction

`editor-types` is the lowest shared crate and contains small protocol-neutral data types.

`editor-core` depends on `editor-types`.

`terminal-backend` depends on `editor-types`.

`config-core` depends on `editor-types`.

`workspace-core` depends on `editor-types` and `config-core`.

`syntax-engine` depends on `editor-types` and the read-only text snapshot API exposed by `editor-core`.

`lsp-client` depends on `editor-types`, `editor-core`, and `config-core`.

`vcs-git` depends on `editor-types` and `workspace-core`.

`vscode-compat` depends on `editor-types` and `config-core`.

`app-ui` depends on model/data APIs from the other crates but never receives their mutable service internals.

The root package depends on all workspace crates and is the only place where services are wired into the event loop.

Cycles between workspace crates are forbidden.

## 5. Root source layout

`src/` contains:

- `main.rs`
- `lib.rs`
- `app/mod.rs`
- `app/action.rs`
- `app/event.rs`
- `app/effect.rs`
- `app/state.rs`
- `app/runtime.rs`
- `app/registry.rs`
- `app/bootstrap.rs`

The top Codex owns these files.

## 6. app-ui source layout

`crates/app-ui/src/` is pre-split into ownership-safe regions:

- `frame/` — shared view/frame abstractions created during foundation.
- `widgets/` — shared primitives owned by the UI shell branch.
- `shell/` — overall layout, tabs, splits, command palette, status and bottom-panel shell.
- `editor/` — editor viewport, gutter, line numbers, overview ruler, selections.
- `workspace/` — Explorer, Quick Open, project search/replace.
- `language/` — Problems, diagnostics decoration, completion, hover, code actions.
- `git/` — Source Control views, diff/staging UI.

`app-ui/src/lib.rs` and module registration are created during foundation and remain top-owned.

## 7. Shared text/location types

The shared type layer uses explicit newtypes for incompatible coordinate systems.

At minimum it defines:

- document identifier,
- character offset,
- logical line/character position,
- screen cell coordinate,
- text range,
- diagnostic range,
- file/workspace identifier,
- command identifier,
- request identifier.

LSP wire positions are contained inside `lsp-client` conversion code and are not used as editor-core positions.

## 8. Rendering boundary

Rendering uses an application-owned virtual framebuffer.

A framebuffer cell contains:

- grapheme/symbol representation,
- foreground role/color,
- background role/color,
- text attributes,
- continuation metadata for wide glyphs when required.

A differential renderer compares the next frame with the last frame and emits only changed terminal cells/regions.

The terminal adapter reports capabilities including true color, 256 color, 16 color, underline styles, enhanced keyboard input, mouse reporting, alternate-screen support, and bracketed paste.

The higher UI layer requests semantic styles; it does not emit ANSI/VT escape sequences.

## 9. Documentation mirror

Documentation mirrors production source paths exactly below `docs/`.

Examples:

- `src/app/runtime.rs` -> `docs/src/app/runtime.md`
- `crates/editor-core/src/buffer/mod.rs` -> `docs/crates/editor-core/src/buffer/mod.md`

Every committed non-generated Rust production source file has one matching Markdown file.

Each source-document file describes:

- role and boundary,
- important public and internal types,
- invariants,
- data flow,
- concurrency assumptions,
- error behavior,
- dependencies on other modules,
- tests that cover it.

Source documentation does not restate source line-by-line.

A repository test verifies the source/document mirror and fails when either side is missing.

## 10. Error model

Libraries return typed errors.

Expected user/environment failures do not panic.

Panics are reserved for violated internal invariants that indicate a programming defect.

External errors are converted at subsystem boundaries into user-visible structured errors and Output logs without leaking terminal state or corrupting buffers.

## 11. Logging

The application uses structured logging.

Logs include subsystem and operation context without logging entire file contents, clipboard contents, credentials, or unsaved source text by default.

The UI Output view can expose safe diagnostics useful for LSP, Git, search, and startup troubleshooting.
