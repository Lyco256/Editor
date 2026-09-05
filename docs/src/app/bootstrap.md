# Process bootstrap

Role: owns process-level startup, positional path parsing, terminal-size fallback, and exit-code
conversion. A path launches the crossterm interactive adapter for a file or workspace; a redirected
launch uses the deterministic headless lifecycle. Invalid options return a usage error without
starting a subprocess. No panic is used for environment failures.
