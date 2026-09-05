# Workspace core boundary

Role: owns the non-UI workspace service boundary for workspace identity, trust, document open/save,
lazy explorer discovery, quick open indexing, project search, file operation planning, and recent
workspace persistence.

The crate is split into focused modules:

- `path.rs` normalizes workspace identities and handles canonical path comparisons.
- `document.rs` loads, decodes, encodes, and atomically saves text documents.
- `filesystem.rs` exposes lazy tree traversal, file-change tracking, quick-open indexing, and file
  operation plans/execution.
- `search.rs` streams project search through `rg` when available and a Rust fallback otherwise.
- `workspace.rs` persists trust and recent-workspace state.

The root application still owns process authorization policy. This crate never reaches into UI types
or process-launch policy bypasses.
