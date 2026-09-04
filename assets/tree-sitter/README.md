# Bundled Tree-sitter assets

This directory holds the offline syntax bundle for the syntax engine.

The current implementation ships the grammar crates through Cargo dependencies and keeps the supported
language inventory here as deterministic metadata. The bundle is intentionally static so no runtime
grammar downloads are required.
