# Process bootstrap

Role: owns process-level startup, positional path parsing, terminal-size fallback, and exit-code
conversion. A path (or an interactive no-argument terminal) launches the crossterm adapter for a file
or workspace; a redirected launch uses the deterministic headless lifecycle. Save effects run in a
workspace adapter thread and return typed completion events. Interactive no-argument startup restores
the latest application-data session (falling back to the current directory), and shutdown persists
the resulting session atomically. Invalid options return a usage error without starting a
subprocess. A panic hook performs best-effort terminal restoration and safe crash logging.
Explorer refreshes are dispatched to the workspace adapter thread and returned as typed events;
filesystem traversal is not performed by the renderer.
