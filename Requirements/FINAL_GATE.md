# Final gate

The Goal run is finished only if all conditions below are mechanically true.

1. `Requirements/001/BASELINE_REF.txt` exists and contains one full Git commit SHA.
2. The frozen-artifact diff against that SHA is empty.
3. `tools/verify-001.ps1` on Windows or `tools/verify-001.sh` on POSIX exits `0`.
4. The verifier prints every ID in `tools/requirements-001/required-cases.txt` exactly once with `PASS`.
5. No required acceptance case is ignored, skipped, conditionally disabled, deleted, renamed, or replaced.
6. `cargo fmt --all -- --check` passes.
7. `cargo clippy --workspace --all-targets --all-features -- -D warnings` passes.
8. `cargo test --workspace --all-features` passes.
9. source/document mirror verification passes.
10. the four final reviewer reports from requirements 071–074 each state `BLOCKERS: 0`.
11. requirement 075's forbidden-pattern and architecture guards pass.
12. the final `devenv` working tree is clean.

A summary saying the app is complete is not evidence. Only the gate above is evidence.
