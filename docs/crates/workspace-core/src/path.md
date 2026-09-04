# Canonical path helpers

Role: normalizes workspace identities and centralizes path comparisons that need to be stable across
case-only differences and separator differences.

Important types:

- `CanonicalPath` stores a normalized workspace path snapshot.
- `PathError` reports missing-path and I/O failures from canonicalization.

Important helpers:

- `canonical_workspace_identity` and `canonical_workspace_key` provide stable trust/workspace keys.
- `path_eq` compares paths using the normalized identity key.
- `is_case_only_rename` identifies the Windows rename edge case.

Invariants:

- Trust persistence uses the canonical workspace key, not a raw user-entered string.
- Path comparison is intentionally platform-aware instead of relying on raw textual equality.

Tests:

- separator and case folding on Windows-style paths.
- case-only rename detection.
