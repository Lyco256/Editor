# Requirements index

Working title: **Editor**

This directory is the authoritative implementation contract for the initial usable release.

The top Codex reads every file in this directory. A subagent reads `00_MASTER_REQUIREMENTS.md`, `01_AGENT_AND_GIT_RULES.md`, `02_ARCHITECTURE_AND_PROJECT_LAYOUT.md`, and the requirement file assigned to its branch.

## Execution order

1. The top Codex completes `03_TOP_FOUNDATION_STAGE_A.md`.
2. The top Codex completes `04_TOP_FOUNDATION_STAGE_B.md`.
3. Wave 1 subagents run in parallel from the same tested `devenv` foundation commit:
   - `10_EDITOR_CORE.md`
   - `11_TERMINAL_BACKEND.md`
   - `12_CONFIG_ENCODING_RECOVERY.md`
   - `13_WORKSPACE_SEARCH_TRUST.md`
   - `14_SYNTAX_ENGINE.md`
   - `15_LSP_CLIENT.md`
   - `16_GIT_BACKEND.md`
   - `17_VSCODE_STATIC_COMPAT.md`
4. The top Codex merges and verifies Wave 1 on `devenv`.
5. Wave 2 subagents run in parallel from the tested Wave 1 `devenv` commit:
   - `20_UI_SHELL_EDITOR.md`
   - `21_UI_WORKSPACE.md`
   - `22_UI_LANGUAGE.md`
   - `23_UI_GIT.md`
6. The top Codex performs `30_INTEGRATION_AND_RELEASE_ACCEPTANCE.md`.
7. Only after every acceptance gate passes is `devenv` merged into `main`.

## File ownership rule

The branch-specific requirement files define writable ownership. Subagents do not modify files outside that ownership. Shared interfaces, root orchestration, workspace membership, the root `Cargo.toml`, `Cargo.lock`, and root `src/app/**` are owned by the top Codex.

If a branch discovers that a shared interface is insufficient, it records the exact required interface change in its completion report and implements against the existing contract as far as possible. It does not independently redesign shared interfaces.
