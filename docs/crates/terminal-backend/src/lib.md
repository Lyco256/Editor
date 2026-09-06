# Terminal backend

Role: owns the terminal-facing boundary for lifecycle management, normalized input, clipboard access,
framebuffer storage, capability detection, and differential rendering.

The crate exports a small adapter surface:

- `TerminalAdapter` for higher-level lifecycle ownership.
- `InputReader` for blocked or polled input consumption.
- `Clipboard` for copy, cut, and paste integration.
- `Framebuffer` and `Cell` for virtual terminal presentation.
- `display_width` and `truncate_display` provide grapheme-aware terminal-cell metrics for chrome.
- `MouseClickTracker` provides deterministic single/double/triple-click normalization.

Data flow stays local to this crate. Higher layers build semantic state, pass it into the framebuffer,
and consume normalized input events. This crate emits escape sequences and platform clipboard calls,
but it does not interpret application actions or mutate UI state.

Error handling is typed. Expected terminal, clipboard, and framebuffer failures return structured
errors instead of panicking. Cleanup is best-effort and idempotent so terminal restoration still runs
when the process exits through normal or error paths.

Tests cover cell replacement, wide and combining grapheme handling, differential rendering, color and
underline fallback, input normalization, clipboard fakes, and the cleanup state machine.
