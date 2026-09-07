# Subagent requirement — language intelligence UI

## Branch

`feat/ui-language`

## Wave

Wave 2. Start only from the tested Wave 1 `devenv` commit.

## Writable ownership

- `crates/app-ui/src/language/**`
- corresponding `docs/crates/app-ui/src/language/**`
- `tests/fixtures/app-ui/language/**`
- `tests/snapshots/ui-language/**`

Do not edit shared UI shell files, root application files, other UI feature directories, or `Cargo.lock`.

## Scope

Implement view/action layers for:

- syntax highlight spans,
- semantic-token overlays,
- diagnostic text decorations,
- diagnostic gutter markers,
- diagnostic overview-ruler markers,
- Problems panel,
- completion popup,
- completion details,
- hover,
- signature help,
- go-to result chooser when multiple locations exist,
- references results,
- rename input/preview state,
- code action / Quick Fix list,
- inlay hints,
- document/workspace symbol chooser,
- formatting action feedback,
- LSP status and restart action.

## Diagnostic visual fallback

The view requests diagnostic underline semantic styles.

The terminal backend performs capability fallback. The language UI remains readable when underline is unavailable.

## Stale data

Every versioned language result is compared with the active document version.

Stale diagnostics, syntax, completion, hover, semantic tokens, and inlay hints are not painted over newer text.

## Problems

Problems groups diagnostics by workspace/file and exposes severity, source, message, location, and related information when present.

Activating a Problem navigates to the location.

## Tests

Framebuffer/state tests cover:

- all four diagnostic severities,
- Problems grouping,
- diagnostic overview markers,
- completion popup,
- hover/signature help,
- rename,
- code actions,
- inlay hints,
- LSP unavailable/crashed/restarting states,
- stale result suppression,
- Unicode ranges.

## Acceptance criteria

- UI does not talk to LSP processes directly.
- Every language action is represented as an application action/effect request.
- Stale results are suppressed.
- `cargo test -p app-ui` passes for language UI cases.
- Clippy passes.
- All owned source files have mirrored documentation.
