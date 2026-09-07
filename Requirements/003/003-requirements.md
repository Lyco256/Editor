# Requirements 003 — Normative Requirements

この文書の **MUST / MUST NOT / SHOULD** は 003 の拘束条件である。

## R003-001 — Source issues are the SSOT

003 の対象は、情報源でユーザーが未解決と確認した不具合を起点にしなければならない。

- 各不具合に `S003-xx` ID を付与すること。
- 各 ID には observed behavior / expected behavior / reproduction / evidence を持たせること。
- 実装都合で issue を別問題へ置き換えてはならない。
- 再現できない issue は「解決済み」ではなく `UNREPRODUCED` と扱うこと。

## R003-002 — Baseline failure is mandatory

各 source issue は、修正コードを入れる前の baseline で失敗することを証明しなければならない。

- 新しい acceptance が baseline でも PASS する場合、その acceptance は対象不具合を捕捉していないため無効。
- baseline FAIL のログ、snapshot、trace、または同等の machine-readable evidence を保存すること。
- baseline の commit SHA を記録すること。

## R003-003 — Product-boundary verification

acceptance は内部 helper や model 単体ではなく、可能な限り実際の product boundary を通すこと。

最低限、対象操作について以下を観測すること。

1. 操作対象 pane
2. pane が表示している buffer / document
3. 操作時に解決された buffer / document
4. cursor / pointer / selection の所属 pane と座標
5. 編集後に変更された buffer / document
6. 非対象 pane / buffer が不変であること

内部状態のみを assert し、画面上／製品上の結果を検証しないテストは 003 の primary acceptance として認めない。

## R003-004 — Secondary pane independence

secondary pane を操作したとき、primary pane の pointer / cursor / buffer binding に誤って依存してはならない。

- secondary pane の表示対象 buffer と操作対象 buffer が一致すること。
- secondary pane の cursor / pointer move が primary pane の位置を暗黙に変更しないこと。
- secondary pane での edit が primary pane の buffer に誤適用されないこと。
- primary / secondary が同一 buffer を表示するケースと別 buffer を表示するケースを分けて検証すること。
- focus を切り替えた直後、split 作成直後、buffer 切替直後でも同じ invariant を満たすこと。

## R003-005 — Same action, same observation

修正前後比較は同一の初期状態、同一の操作列、同一の観測点で行うこと。

修正後だけ追加の focus 操作、refresh、reopen、buffer rebind 等を必要とする場合、それは原則として修正成功とみなさない。

## R003-006 — End-to-end issue matrix

各 `S003-xx` は次の全列を埋めること。

| Field | Required |
|---|---|
| Source issue ID | MUST |
| User-visible symptom | MUST |
| Minimal reproduction | MUST |
| Expected result | MUST |
| Baseline actual result | MUST |
| Baseline evidence | MUST |
| Suspected layer | SHOULD |
| Fix commit / diff | MUST |
| Post-fix actual result | MUST |
| Post-fix evidence | MUST |
| Regression test ID | MUST |
| Final status | MUST |

空欄がある issue は CLOSED にできない。

## R003-007 — No self-fulfilling checker

実装と checker が同一の誤った仮定を共有して PASS することを防ぐ。

- canonical checker の期待値を production implementation から生成してはならない。
- acceptance の expected state を production resolver と同一ロジックで計算してはならない。
- oracle は独立した入力→期待結果表、またはユーザー可視 invariant から判定すること。

## R003-008 — Failure-message quality

acceptance が失敗した場合、少なくとも以下を出力すること。

- source issue ID
- step number
- active/focused pane
- displayed buffer
- resolved operation buffer
- cursor / pointer owner and position
- expected value
- actual value

単なる `assertion failed` のみは禁止する。

## R003-009 — Regression preservation

003 修正後も、001 と 002 の有効な acceptance は維持しなければならない。

ただし、003 で既存 acceptance 自体が実製品挙動を誤って表していると判明した場合は、理由と before/after を記録した上で acceptance を修正できる。

「既存 acceptance を壊さないために実際の不具合を残す」ことは禁止する。

## R003-010 — Completion gate is fail-closed

以下のいずれか一つでも満たさない場合、verify-003 は非 0 で終了しなければならない。

- source issue が全件 `PASS/CLOSED`
- baseline FAIL evidence が全件存在
- post-fix PASS evidence が全件存在
- product-boundary acceptance が全件 PASS
- 001 regression PASS
- 002 regression PASS
- workspace tests PASS
- format / lint PASS
- docs / requirement mirror consistency PASS
- working tree clean（検証対象を commit した最終モード）

## R003-011 — Human-observable parity

自動 acceptance の結果と、ユーザーが同じ操作を行ったときの結果が一致する必要がある。

自動テストだけ PASS し、人間操作で同じ不具合が残る場合、テスト側の defect とみなし 003 を再オープンする。

## R003-012 — No completion by review text

reviewer / oracle / product review の `CONFLICTS: 0` / `BLOCKERS: 0` は補助証拠であり、実製品 acceptance の代替にはならない。

003 の最終判定は executable acceptance と evidence matrix を優先する。
