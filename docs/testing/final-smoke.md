# Windows Terminal smoke test

An interactive PTY-backed Windows smoke run was completed on 2026-09-05 using the release binary.
Verified manually: no-argument startup, file opening, keyboard insertion, dirty-quit protection,
undo, Command Palette filtering/activation, vertical split, Explorer toggle, Output toggle, clean
exit, and terminal restoration. The process exited with code 0 and restored the terminal modes.

Mouse selection, system clipboard, resize events, diagnostic fallback, and Git actions were not
available for direct PTY interaction in this session; their deterministic framebuffer/backend and
trust-gating scenarios remain covered by the automated suite. These items still require a native
Windows Terminal pass before release sign-off.
