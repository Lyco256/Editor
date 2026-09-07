# Requirements 003

003 は、Requirements 002 がすべての内部検証を PASS したにもかかわらず実際のユーザー可視不具合が改善されなかった問題を是正するための requirement set である。

## Documents

- `003-goal.md` — 背景、Goal、Non-goals、完了定義
- `003-requirements.md` — MUST / MUST NOT を含む規範要件
- `003-source-issues.md` — 情報源の問題と Source Issue Matrix
- `003-acceptance.md` — baseline RED → fix → same-scenario GREEN の acceptance
- `003-verification.md` — verify-003 の fail-closed contract
- `003-implementation-constraints.md` — root-cause / ownership / traceability 制約

## Core rule

> 既存テスト群が全部 PASS しても、ユーザーが同じ操作で同じ不具合を再現できるなら 003 は FAIL である。

003 の primary evidence は「実製品上で source issue が再現しなくなったこと」であり、review 文言や unit-test count ではない。
