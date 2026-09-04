# Executable entry point

Role: transfers process control to bootstrap and returns its exit status. It contains no service or UI
logic. Startup failures remain non-panicking and become a failing process status. Covered by bootstrap
and headless runtime tests.

