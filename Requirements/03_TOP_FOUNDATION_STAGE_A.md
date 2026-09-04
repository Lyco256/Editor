# Top Codex — foundation stage A: repository contract

## Goal

Create the repository, build, test, documentation, branch, and workspace structure that all subagents can use without editing shared root files.

## Required work

The top Codex:

- confirms the current integration branch is `devenv`,
- creates `main` at the clean pre-feature baseline if `main` is absent,
- returns to `devenv`,
- creates the complete Cargo workspace and every required crate,
- pins the exact current stable Rust toolchain in `rust-toolchain.toml`,
- uses Rust edition 2024,
- creates root formatting and lint configuration,
- configures the root release profile to `opt-level = "s"`, fat LTO, one codegen unit, symbol stripping, and panic abort,
- creates `.gitignore`,
- creates `AGENTS.md` containing the branch/ownership/test rules needed by Codex agents,
- creates `src`, `crates`, `tests`, `docs`, `Requirements`, `tools`, `build`, and `assets` structure,
- creates compilable crate/module placeholders for every required ownership region,
- creates the source-document mirror checker,
- creates Windows and POSIX verification scripts,
- commits an authoritative `Cargo.lock`.

`build/` contains packaging definitions/scripts only. Cargo output remains in `target/`.

## Required test stack

The workspace standardizes on:

- Rust built-in test harness for unit and integration tests,
- `rstest` for parameterized cases,
- `proptest` for stateful/property-sensitive text, mapping, parser-boundary, and encoding cases,
- `insta` for deterministic framebuffer/view snapshots,
- `tempfile` for isolated filesystem and Git repositories,
- Tokio test support for async subsystem tests.

These test tools are included only where used.

## Verification scripts

`tools/verify.ps1` and `tools/verify.sh` run the authoritative local verification sequence:

1. format check,
2. workspace Clippy with warnings denied,
3. workspace tests,
4. documentation mirror test.

The scripts stop on the first failure and return a non-zero exit code.

## Acceptance criteria

Stage A is complete only when:

- `main` and `devenv` have the intended roles,
- `devenv` is clean,
- every workspace crate compiles,
- all root tests pass,
- the documentation mirror checker passes,
- the verification scripts run successfully in the current environment,
- root `Cargo.lock` is committed,
- all subagent-owned directories already exist so subagents do not need to edit root module registration or workspace membership.
