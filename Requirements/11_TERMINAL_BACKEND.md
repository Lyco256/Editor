# Subagent requirement — terminal-backend

## Branch

`feat/terminal-backend`

## Writable ownership

- `crates/terminal-backend/**`
- `docs/crates/terminal-backend/**`
- `tests/fixtures/terminal-backend/**`

Do not edit root application files, shared types, other crates, or `Cargo.lock`.

## Scope

Implement terminal lifecycle, input normalization, framebuffer, differential rendering, capability fallback, and system clipboard adapters. Use `crossterm` for portable raw-mode/input/terminal primitives and direct VT emission inside this crate for capabilities that `crossterm` does not expose with sufficient fidelity.

Required behavior:

- Windows Terminal Tier 1.
- Modern ANSI/VT Linux terminals Tier 2.
- alternate screen,
- raw mode,
- cursor show/hide/shape primitives,
- terminal resize events,
- bracketed paste,
- keyboard normalization,
- mouse reporting and normalization,
- scroll wheel,
- clipboard copy/cut/paste integration,
- virtual framebuffer,
- wide glyph handling,
- combining-sequence-safe frame behavior,
- differential frame output,
- true-color rendering,
- 256-color quantization,
- 16-color quantization,
- underline capability fallback,
- clean terminal restoration.

Higher layers supply semantic styles and normalized actions. They do not emit escape sequences directly.

## Windows behavior

Windows uses VT-capable terminal behavior as the main path. Native Windows APIs are used for the Tier 1 system clipboard and where required for reliable console mode/capability integration. Linux Tier 2 uses a system clipboard provider when a graphical clipboard service is available and degrades to a clearly reported unavailable system-clipboard state when the host session exposes none.

Windows Terminal is the reference for:

- true color,
- mouse,
- bracketed paste where exposed,
- colored underline,
- curly underline where supported.

## Fallback

The backend reports capabilities. Unsupported capabilities degrade in this order rather than failing:

- true color -> 256 color -> 16 color,
- curly underline -> straight underline -> style emphasis,
- enhanced key reporting -> normalized legacy key mapping.

## Tests

Required tests include:

- framebuffer cell replacement,
- wide glyph continuation behavior,
- diff renderer emits no output for identical frames,
- small frame edits emit bounded changes rather than full redraw,
- color quantization determinism,
- style fallback,
- key normalization,
- mouse normalization,
- resize,
- clipboard adapter fake,
- cleanup state machine.

Snapshot tests use a fake terminal writer. They do not require visual inspection.

## Acceptance criteria

- Real terminal I/O is confined to this crate.
- App/UI crates can be tested with a fake backend.
- An unchanged frame causes no terminal redraw payload.
- Terminal cleanup is idempotent.
- `cargo test -p terminal-backend` passes.
- Clippy with denied warnings passes.
- All owned source files have mirrored documentation.
