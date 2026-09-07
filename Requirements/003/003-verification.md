# Requirements 003 — Verification Contract

## 1. verify-003 responsibility

`tools/verify-003.ps1`（または既存命名規則に合う同等スクリプト）は、003 の完了条件を fail-closed で評価する。

verify-003 自体が production behavior の代替 oracle になってはならない。verify-003 は各独立 acceptance / regression / evidence の結果を集約する。

## 2. Required phases

### Phase 1 — Repository state

- target commit SHA を出力
- clean / dirty を出力
- final mode では dirty tree を FAIL

### Phase 2 — Source Issue Matrix validation

- 全 `S003-*` を列挙
- required fields の空欄を FAIL
- `OPEN` / `UNREPRODUCED` / `FIXED_UNVERIFIED` が 1 件でもあれば FAIL

### Phase 3 — Baseline evidence validation

各 issue に baseline commit と baseline FAIL evidence があることを確認する。

fix 後の結果しか存在しない issue は FAIL。

### Phase 4 — 003 product acceptance

A003 を実行し、issue ID ごとに結果を表示する。

例:

```text
A003-02 / S003-02 : PASS
  focused_pane      = secondary#2
  displayed_buffer  = buffer:Y
  resolved_buffer   = buffer:Y
  pointer_owner     = secondary#2
  changed_buffers   = [Y]
```

失敗時は expected / actual を同時表示する。

### Phase 5 — 001 / 002 regression

既存 acceptance を skip/ignore なしで実行する。

### Phase 6 — Workspace quality gates

- workspace tests
- fmt
- Clippy / lint
- docs mirror / consistency

### Phase 7 — Evidence cross-check

各 S003 issue に対して以下が 1:1 で存在することを確認する。

- baseline FAIL
- fix diff / commit
- post-fix PASS
- regression test

### Phase 8 — Final summary

成功時のみ以下を出す。

```text
REQUIREMENTS 003: PASS
SOURCE ISSUES: N/N CLOSED
BASELINE RED EVIDENCE: N/N
PRODUCT ACCEPTANCE: PASS
001 REGRESSION: PASS
002 REGRESSION: PASS
WORKSPACE TESTS: PASS
FMT/LINT: PASS
EVIDENCE CROSS-CHECK: PASS
WORKTREE: CLEAN
```

## 3. Forbidden shortcuts

次は禁止する。

- baseline FAIL を省略
- source issue を matrix から削除して PASS にする
- expected value を production helper から生成
- failing case の skip / ignore / xfail 化
- 実製品 acceptance を unit test の PASS で代替
- review 文言のみで gate を満たす
- final verifier の一部フェーズ失敗を warning 扱いにする

## 4. Oracle independence

003 oracle は次の優先順位で expected behavior を決める。

1. 情報源の user-visible expected behavior
2. Requirements 003 に明記した invariant
3. 独立 fixture の expected state

production implementation の current behavior は expected behavior の根拠にしてはならない。

## 5. Completion decision

003 は verifier の「総合 PASS」に加え、Source Issue Matrix が全件 CLOSED である場合のみ完了できる。

一つでも実操作で再発した場合、その時点で 003 は未完了または再オープン扱いとする。
