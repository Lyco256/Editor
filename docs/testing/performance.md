# Performance measurements

Reference environment: Windows x64, Rust 1.98.1, release profile (`opt-level=s`, fat LTO,
single codegen unit, stripped symbols, panic abort). The release executable measured **549,888 bytes**
(`target/release/editor.exe`) on 2026-09-05; no mandatory bundled assets are currently shipped.

Resident-memory, idle-CPU, 10 MiB typing-latency, and repeated open/close observations require an
interactive Windows Terminal profiling session unavailable to this headless run. They remain explicit
follow-up measurements rather than being guessed or reported as passing. A budget exception must
include a measured limitation and explicit user approval.
