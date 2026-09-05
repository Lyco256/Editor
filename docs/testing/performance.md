# Performance measurements

Reference environment: Windows x64, Rust 1.98.1, release profile (`opt-level=s`, fat LTO,
single codegen unit, stripped symbols, panic abort). The release executable measured **1,241,088 bytes**
(`target/release/editor.exe`) on 2026-09-05; no mandatory bundled assets are currently shipped.
The Inno Setup package compiled successfully and measured **3,896,263 bytes**
(`target/package/Editor-0.1.0-setup.exe`).
Redirected no-argument startup measured approximately **48 ms** on this host. This is a headless
lifecycle measurement and does not substitute for interactive idle profiling.

The interactive PTY sample measured approximately **7.5 MiB working set** (7,897,088 bytes) and
**1.7 MiB private memory** while idle. Two seconds later the process CPU counter was unchanged at
0.06 s (no measurable CPU increase in that sample). Five redirected release launches completed in
157.51 ms total (**31.50 ms average**), covering repeated startup/close lifecycle overhead.

The deterministic editor-core 10 MiB edit transaction completed in **21.26 ms** on the same host,
below the 250 ms automated budget. This measures the bounded text-edit path without terminal I/O;
native keystroke-to-frame latency is still not directly profiled.

10 MiB typing latency and repeated open/close of a 10 MiB document still require a dedicated native
Windows Terminal profiling session; they are not inferred from the startup measurements above.
