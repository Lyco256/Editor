# Top Codex runbook

## Objective

Repair the current `devenv` implementation so terminal resize, cursor/navigation, mouse interaction, selection/caret rendering, workbench layout, tabs, Explorer, and related projections behave as specified in `Requirements/001/`.

For behavior that Microsoft Edit implements and Editor also needs, Microsoft Edit is the behavioral oracle. Do not invent different primitive interaction behavior because another design appears easier.

For Editor-only capabilities absent from Microsoft Edit, such as multiple cursors, split editor groups, VS Code preview tabs, LSP overlays, Git markers, and project-search markers, follow the exact requirements in this cycle.

## Two-run execution model

### Run A — acceptance setup, normal mode

Use `Requirements/GOAL_SETUP_PROMPT.md`.

Run A must:

1. verify the repository is on `devenv`;
2. inspect current production code and existing tests;
3. implement only the acceptance harness described in requirements 004 and 005;
4. commit the frozen acceptance artifacts;
5. record the frozen commit SHA in `Requirements/001/BASELINE_REF.txt` in a second commit;
6. prove existing non-001 tests still run;
7. prove at least the known-broken 001 acceptance cases fail before fixes;
8. stop without fixing production behavior.

Run A must not opportunistically fix resize, cursor, mouse, layout, or navigation behavior.

### Run B — Goal Mode implementation

Start `/goal` using the exact text from `Requirements/GOAL_PROMPT.md`.

The Goal run:

1. verifies the frozen baseline before any implementation;
2. executes top-owned foundation tasks 006 and 010–015 sequentially;
3. creates agent worktrees only after the foundation is green;
4. runs the parallel lanes defined in `Requirements/AGENT_ORCHESTRATION.md`;
5. merges only green lane branches into `devenv`;
6. executes top-owned integration tasks 050–053 and 070;
7. runs reviewer tasks 071–074;
8. fixes every blocker found by reviewers;
9. repeats review until all blocker counts are zero;
10. runs requirement 075;
11. stops only when `tools/verify-001` exits zero and requirement 076 is satisfied.

## Branch rules

- `main` remains stable.
- `devenv` is the integration branch and is the starting branch.
- implementation branches use `feat/001-*`.
- only the top Codex merges into `devenv`.
- subagents never merge another feature branch into their worktree.
- no force push or history rewrite on `main` or `devenv`.
- `Cargo.lock` is top-owned during integration.

## Failure policy

A failing requirement is fixed; it is not reclassified as optional.

A difficult requirement is not removed.

A failing acceptance test is not edited to match the implementation.

A terminal limitation may change only the low-level representation explicitly allowed by a requirement; it does not allow the user-visible behavior to be dropped.

If a repository-local automated test cannot execute because of a missing external binary, the implementation supplies a fake/test seam unless the requirement explicitly names that external binary as mandatory.

## Completion report

Write the final report to `docs/testing/requirements-001-final.md`.

It contains:

- frozen baseline SHA;
- final implementation SHA;
- verifier command and zero exit status;
- every acceptance case ID and PASS;
- reviewer reports and zero-blocker status;
- a list of production modules changed;
- confirmation that frozen acceptance artifacts are unchanged from the baseline.
