# 007 — independent pre-freeze reviews

Run two fresh read-only reviewer agents before freeze.

Reviewer A writes `docs/testing/requirements-002-review-vector.md`. It manually derives K004/K005 and both K006 variants, runs the independent checker, compares vector to written preferred-column invariant and existing canonical editor-core tests, and ends exactly `CONFLICTS: N`.

Reviewer B writes `docs/testing/requirements-002-review-adapter.md`. It inspects the modified Rust adapter and pre-migration diff, verifies the adapter reads vectors, invokes real production code, asserts all checkpoints, no other acceptance case was weakened, and no production code changed in setup. It ends exactly `CONFLICTS: N`.

Both must end `CONFLICTS: 0`. Otherwise no baseline freeze and no Goal Mode.
