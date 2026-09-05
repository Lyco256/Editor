# Performance measurements

Reference environment: Windows x64, Rust 1.98.1, release profile (`opt-level=s`, fat LTO,
single codegen unit, stripped symbols, panic abort). The release executable measured
**18,370,560 bytes** (`target/release/editor.exe`) on 2026-09-05 after the current integration
build; no mandatory bundled assets are currently shipped.
The Inno Setup package compiled successfully and measured **3,896,263 bytes**
(`target/package/Editor-0.1.0-setup.exe`).
Five redirected no-argument launches measured 50.24, 28.13, 26.58, 25.42, and 25.90 ms
(**31.26 ms average**) on this host. This is a headless lifecycle measurement and does not
substitute for interactive idle profiling.

The interactive PTY sample measured approximately **7.5 MiB working set** (7,897,088 bytes) and
**1.7 MiB private memory** while idle. Two seconds later the process CPU counter was unchanged at
0.06 s (no measurable CPU increase in that sample). Five redirected release launches completed in
157.51 ms total (**31.50 ms average**), covering repeated startup/close lifecycle overhead.

The deterministic editor-core 10 MiB edit transaction completed in **21.22 ms** on the same host,
below the 250 ms automated budget. This measures the bounded text-edit path without terminal I/O;
native keystroke-to-frame latency is still not directly profiled.

10 MiB typing latency and repeated open/close of a 10 MiB document still require a dedicated native
Windows Terminal profiling session; they are not inferred from the startup measurements above.

A process-level Windows Terminal launch loop was additionally run on 2026-09-05 for five iterations
against a 10 MiB document. The exact `target/release/editor.exe` process was sampled after 700 ms
and then force-closed to avoid leaving test tabs open; working-set samples were 122,736,640,
99,835,904, 119,201,792, 130,641,920, and 95,600,640 bytes (minimum 91.2 MiB, maximum
124.6 MiB, mean 108.3 MiB). The samples do not show monotonic growth, but force-close is not a
clean interactive close and therefore does not replace a native keystroke-to-frame or clean
open/close profiling session.
