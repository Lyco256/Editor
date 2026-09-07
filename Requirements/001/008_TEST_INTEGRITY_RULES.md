# 008 — test-integrity rules

## Forbidden

No agent may make a failing acceptance case pass by:

- deleting it;
- renaming its case ID;
- adding `#[ignore]`;
- adding a cfg that normally excludes it;
- loosening the expected value;
- replacing exact behavior with “non-empty”, “does not panic”, or other weaker assertions;
- mocking the production layer being tested instead of its external dependency;
- changing the verifier to omit it;
- editing `required-cases.txt`;
- changing the baseline reference.

## Required

Tests may use fakes only at external boundaries:

- terminal transport,
- time/clock,
- LSP process,
- Git process/network portions,
- filesystem temporary roots.

Core editor math, workbench layout, hit testing, state routing, and rendering paths under test must be the real production implementation.

A bugfix adds non-frozen lower-level unit/property tests in addition to the frozen acceptance test when useful.

No acceptance claim relies only on snapshot text when a behavioral state assertion can also be made.
