# Agent, worktree, branch, and merge rules

## 1. Branch topology

The repository has three branch roles:

- `main`: stable integrated state.
- `devenv`: active integration state.
- `feat/<scope>-<name>`: isolated subagent implementation branches.

The repository initially opens on `devenv`.

If `main` does not exist at the start, the top Codex creates `main` at the clean baseline commit that precedes feature implementation, then returns to `devenv`.

No subagent works directly on `main` or `devenv`.

## 2. Worktrees

Every subagent receives exactly one worktree and one `feat/...` branch.

Only the top Codex coordinates repository-writing subagents/worktrees. Feature subagents do not spawn additional agents that modify the repository.

All Wave 1 feature branches start from the exact tested foundation commit on `devenv`.

All Wave 2 feature branches start from the exact tested Wave 1 integration commit on `devenv`.

A subagent does not merge another branch into its worktree.

A subagent does not rebase its branch unless instructed by the top Codex after an integration conflict.

## 3. Ownership

Each subagent requirement file defines writable paths.

A subagent:

- modifies only its writable paths,
- adds tests only in its owned test paths,
- adds source documentation only in matching owned documentation paths,
- does not edit root orchestration,
- does not edit shared protocol contracts,
- does not edit the root workspace member list,
- does not edit the root `Cargo.toml`,
- does not commit `Cargo.lock`.

Cargo commands may update `Cargo.lock` locally. The subagent reverts only `Cargo.lock` before its final commit. The top Codex regenerates and commits the single authoritative lockfile after merges.

## 4. Shared-interface changes

`crates/editor-types/**`, root `src/app/**`, and shared public contracts created during foundation are top-Codex-owned.

If an implementation cannot meet its requirement through the existing shared contract, the subagent does not change the contract. Its completion report contains:

- the exact missing capability,
- the smallest signature/type change required,
- the caller and callee affected,
- the test that would verify the change.

The top Codex applies interface changes on `devenv`, reruns foundation tests, and only then requests a branch follow-up if required.

## 5. Feature-branch completion gate

A feature branch is mergeable only when all are true:

- owned source code is complete,
- no placeholder implementation remains,
- no `todo!()`, `unimplemented!()`, temporary panic, or disabled test remains in owned production paths,
- owned documentation mirrors every owned production source file,
- crate-specific formatting passes,
- crate-specific Clippy passes with warnings denied,
- crate-specific tests pass,
- required integration fixtures for the feature are committed,
- the branch working tree is clean.

## 6. Merge rules

The top Codex is the only agent that merges feature branches.

Feature branches are merged into `devenv` with non-fast-forward merge commits so the integration history preserves feature boundaries.

After each merge group, the top Codex regenerates `Cargo.lock`, compiles the whole workspace, and runs the tests required by the integration stage.

A failed merge or failed test blocks further dependent Wave 2 work until `devenv` is green again.

`main` receives only the fully tested `devenv` state after `30_INTEGRATION_AND_RELEASE_ACCEPTANCE.md` passes.

No force push, history rewrite, or reset that discards committed work is permitted on `main` or `devenv`.

## 7. Integration fixes

The top Codex owns integration glue and cross-feature fixes on `devenv`.

An integration fix must not silently move a feature requirement out of scope. It either fixes the defect or records a user-visible blocker.

## 8. Commit quality

Each feature branch uses focused commits that keep production code, its tests, and its mirrored source documentation together.

Generated build outputs, editor temporary files, test scratch directories, and worktree metadata are not committed.
