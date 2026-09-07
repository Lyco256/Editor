# Requirements 003 — Source Issue Matrix

## Status vocabulary

- `OPEN` — 再現済み・未修正
- `UNREPRODUCED` — 情報源には存在するが、まだ baseline で再現できていない
- `FIXED_UNVERIFIED` — 修正候補あり・実製品 acceptance 未完了
- `PASS` — baseline FAIL と post-fix PASS を同一手順で確認済み
- `CLOSED` — PASS + regression + evidence 完了

## S003-01 — 002 completed but user-visible behavior did not change

**Observed behavior**  
Requirements 002 が全 verifier / checker / review を PASS して完了扱いになったにもかかわらず、ユーザーが確認していた実際の不具合は改善されなかった。

**Expected behavior**  
Requirements を完了扱いにするための acceptance は、実際の製品挙動の改善と一致しなければならない。

**003 requirement**  
003 では対象不具合ごとに baseline FAIL → fix → same-scenario PASS を必須化し、既存テスト群だけの PASS で完了できないようにする。

**Initial status**: `OPEN`

---

## S003-02 — Secondary pane behavior remains incorrect despite pane-buffer/pointer fix

**Observed behavior**  
secondary pane の pointer 分離／pane-buffer 解決に関する修正と回帰テストが入ったと報告されたが、ユーザーが実際に操作した際の不具合は変化しなかった。

**Expected behavior**  
secondary pane に対する操作は、その pane が表示している buffer と、その pane 自身の cursor / pointer state に対して行われる。primary pane の状態へ誤って解決されてはならない。

**Required reproduction families**

1. primary / secondary が異なる buffer を表示
2. primary / secondary が同一 buffer を別位置で表示
3. secondary に focus を移した直後
4. secondary の buffer を変更した直後
5. split 作成直後
6. primary → secondary → primary → secondary と focus を往復した後

各ケースで、move / selection / edit のうち対象機能が実際に使う操作を実行し、displayed buffer / resolved buffer / pointer owner / edit target を記録する。

**Pass condition**  
全ケースで操作対象 pane と実際の状態変更先が一致し、非対象 pane / buffer に意図しない変更がない。

**Initial status**: `OPEN`

---

## S003-03 — Test oracle can pass while checking the wrong abstraction

**Observed behavior**  
002 では canonical checker、acceptance、oracle review、product review が PASS しても実不具合を捕捉できなかった。

**Expected behavior**  
checker の PASS はユーザー可視の正しい結果を直接または独立 oracle で確認している必要がある。

**003 requirement**  
production resolver と同じ helper を expected-value 計算に使わず、外部観測可能な invariant から expected state を固定する。

**Initial status**: `OPEN`

---

## Source additions

情報源に他の未解決不具合が存在する場合、実装開始前に `S003-04` 以降へ追加する。追加項目は最低でも以下を記載する。

- exact symptom
- minimal reproduction
- expected behavior
- baseline actual behavior
- affected pane/buffer/input sequence
- evidence reference

情報源にある issue を matrix へ載せずに 003 の対象外とする場合、明示的な理由を Goal または Non-goals に追記する。
