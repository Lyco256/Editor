# Requirements 002 — oracle reconciliation and continuation

This cycle exists because the 001 Goal correctly stopped on an invalid frozen oracle. Preserve the current clean local `devenv`; do not reset or discard the implementation produced by the blocked 001 Goal.

002 supersedes 001 only for acceptance-oracle governance, the incorrect K004/K005/K006 frozen fixtures/expectations, and baseline-integrity verification after the authorized migration. All other 001 product requirements remain active.

## New non-negotiable policy

Codex must never invent a frozen acceptance fixture or expected value from prose. Deterministic cases use requirement-authored canonical machine-readable vectors. Rust acceptance code is only an adapter that reads those vectors and executes production APIs. Before freezing, an independent reference checker and two fresh read-only reviewers must report zero conflicts.

## Run order

1. Normal mode: give the top Codex `Requirements/002_SETUP_PROMPT.md`. It executes 002/000–010 only and stops after creating `Requirements/002/BASELINE_REF.txt`.
2. Goal Mode: only after setup is green, use exactly `Requirements/002_GOAL_PROMPT.md`.

Goal Mode may not modify the frozen 002 specification, vectors, reference checker, repaired acceptance adapter, verifier, or baseline.
