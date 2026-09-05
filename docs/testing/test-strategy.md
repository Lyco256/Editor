# Test strategy

Unit tests cover typed domain invariants and boundary error mapping. Integration tests use temporary
directories, fake LSP/Git processes, deterministic framebuffer snapshots, and headless terminal
adapters. Property tests exercise Unicode coordinates, transactions, encoding, and parser boundaries.
No normal test requires a network remote or disables a failing assertion.
`tools/verify.ps1` additionally performs the release build and launches the redirected binary to
catch packaging or startup regressions before promotion.
