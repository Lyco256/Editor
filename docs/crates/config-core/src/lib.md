# Configuration core boundary

## Role and boundary

This crate is the process-free boundary for settings, EditorConfig, text encoding, line endings,
session persistence, and unsaved-buffer recovery. It re-exports the stable public models from the
specialized modules. `LargeFileSettings` retains the mandatory 32 MiB default.

## Invariants and data flow

Configuration is parsed as data and never executes commands. Encoding metadata and recovery state
flow upward to application orchestration; this crate does not mutate editor buffers or workspace
files. Expected filesystem, syntax, conversion, and persistence failures use typed errors.
`load_settings_or_default` makes a missing `.vscode/settings.json` a clean default while surfacing
malformed or unreadable settings as diagnosable issues.

## Dependencies and tests

The implementation uses Serde for durable data and dedicated encoding modules for conversion.
Module tests cover the behavior behind each export, while the repository documentation-mirror test
keeps this file paired with `crates/config-core/src/lib.rs`.
