# Requirements 003 — Acceptance Specification

## A003-00 — Acceptance philosophy

003 の acceptance は「コードが意図した構造になっているか」ではなく「ユーザー操作に対して製品が正しい結果を返すか」を判定する。

すべての primary acceptance は source issue ID に紐付ける。

## A003-01 — Red baseline gate

### Procedure

1. 003 修正前の baseline commit を checkout する。
2. S003-02 の minimal reproduction を実行する。
3. pane / buffer / pointer / edit target の観測値を取得する。
4. expected state と比較する。

### Expected

- 少なくとも S003-02 を捕捉するケースが FAIL する。
- FAIL が source symptom と一致する。

### Failure of the acceptance itself

baseline で全ケース PASS の場合、003 acceptance は不十分として FAIL とする。

---

## A003-02 — Different-buffer secondary pane

### Setup

- Pane A = primary
- Pane B = secondary
- Buffer X と Buffer Y は内容が識別可能
- Pane A は Buffer X
- Pane B は Buffer Y

### Action

1. Pane B を focus
2. Pane B で cursor/pointer を移動
3. Pane B で識別可能な edit を 1 回実行

### Required observations

- focused pane = B
- displayed buffer(B) = Y
- resolved operation buffer = Y
- pointer owner = B
- Buffer Y のみが expected diff を持つ
- Buffer X は不変

---

## A003-03 — Same-buffer independent view state

### Setup

- Pane A と Pane B が同じ Buffer X を表示
- Pane A と Pane B の cursor / viewport / pointer position は異なる初期値

### Action

1. Pane B を focus
2. Pane B の pointer/cursor を移動

### Expected

- Pane B の view-local state が更新される
- Pane A の view-local state は仕様上共有すべき項目を除き不変
- edit が発生する場合、Buffer X の content change は 1 回だけ
- pane-local pointer identity が取り違えられない

---

## A003-04 — Focus-switch stress sequence

### Sequence

`A → B → A → B`

各 focus 後に対象 pane で pointer/cursor move を行い、最後の B で edit を実行する。

### Expected

各 step の resolved pane / buffer が現在 focus と一致し、古い pane state が残留しない。

---

## A003-05 — Buffer-switch immediately before edit

### Setup

Pane B の表示 buffer を Y から Z に切り替える。

### Action

buffer switch の直後、追加の focus refresh 等を挟まず edit を行う。

### Expected

- displayed buffer(B) = Z
- resolved operation buffer = Z
- Z のみ変更
- Y と Pane A の buffer は不変

---

## A003-06 — Split-creation immediate interaction

### Action

1. split を作成
2. 新しい secondary pane を対象にする
3. 追加の reopen / refresh をせず最初の pointer/cursor operation を実行
4. edit を実行

### Expected

新規 pane の初期 binding が一貫し、旧 pane の pointer/buffer state を誤参照しない。

---

## A003-07 — Non-target immutability

すべての S003-02 ケースで、対象外 pane/buffer の state snapshot を action 前後で比較する。

許可された shared state 以外に差分があれば FAIL。

---

## A003-08 — Fix effectiveness

fix commit 上で A003-01〜07 を同じ fixtures / inputs で再実行する。

### Expected

- baseline で source symptom を示したケースが PASS へ変化する。
- expected result を修正後コードに合わせて書き換えることは禁止。

---

## A003-09 — Regression gate

以下を実行し全件 PASS を要求する。

- Requirements 001 acceptance
- Requirements 002 acceptance
- 003 product-boundary acceptance
- workspace tests
- formatting
- lint / Clippy
- docs / mirrored requirements consistency

skip / ignore / xfail を新規導入して PASS 数を作ることは禁止する。

---

## A003-10 — Manual parity spot-check

自動 acceptance と同じ fixture / operation sequence を実際の CLI product で実行し、少なくとも S003-02 の代表ケースを人間が観測できる形で確認する。

### Evidence

以下のいずれかを保存する。

- deterministic terminal transcript
- event/state trace
- snapshot sequence
- screen recording / equivalent artifact（環境で可能な場合）

「自動テストは PASS したが実操作では変わらない」場合は即 FAIL。
