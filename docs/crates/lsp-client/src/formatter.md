# Formatter runner boundary

Role: executes a structured, already-authorized formatter command and turns its stdout into one
whole-document replacement edit.

Important types:

- `FormatterRunner` starts the child process, feeds the snapshot text to stdin, and gathers stdout,
  stderr, and exit status.
- `FormatterSpec` carries the executable path, structured arguments, environment, and timeout.
- `FormatterOutcome` returns the replacement range, replacement text, captured stdout/stderr, and
  the exit code.
- `ProcessCancelToken` gives higher layers a structured cancellation handle.

Invariants:

- The runner never decides trust or authorization policy.
- The command is invoked with structured arguments only; no shell concatenation is used.
- A successful run returns exactly one whole-document replacement edit.
- Timeouts and cancellations terminate the child process and surface typed failures.

Data flow:

- The caller passes a frozen `TextSnapshot`.
- The runner writes the snapshot bytes to the formatter stdin and waits for process completion.
- On success the stdout bytes become the replacement text.

Concurrency and error behavior:

- Tokio tasks collect stdout and stderr while the process is running.
- Invalid UTF-8, spawn failures, non-zero exits, cancellations, and timeouts are all reported as
  typed errors.

Tests:

- Integration tests cover uppercase success, timeout, non-zero exit, malformed output, and
  cancellation against the fixture formatter mode.
