# Requirements 003 — Goal

## 1. 背景

Requirements 002 は、検証スクリプト、canonical checker、001 acceptance、workspace tests、fmt、Clippy、docs mirror、oracle review、product review などがすべて PASS し、`CONFLICTS: 0` / `BLOCKERS: 0` と判定されたにもかかわらず、実際の製品操作ではユーザーが確認していた不具合が改善されなかった。

特に secondary pane 周辺では pane-buffer 解決やポインター分離に関する内部修正と回帰テストが追加されたが、ユーザーが観測する挙動には変化がなかった。これは「内部モデル／テストの整合性」と「実際の製品挙動」が分離していたことを示す。

003 は個別の内部実装を正しいと仮定せず、ユーザーが再現できる実製品挙動を唯一の完了基準に据える。

## 2. Goal

003 の Goal は、情報源で確認された未解決のユーザー可視不具合について、実製品上の再現手順を固定し、修正前に失敗し、修正後に同一手順で成功する自動／半自動 acceptance を構築したうえで、実際の挙動を修正することである。

003 は次を満たして初めて完了とする。

1. 情報源にある対象不具合を Source Issue Matrix に列挙し、各項目に一意な ID を付与する。
2. 各不具合について「修正前の実製品」で FAIL する再現手順または executable acceptance を用意する。
3. 実装変更後、同一の入力・操作・観測点で PASS する。
4. PASS 判定は内部データ構造だけでなく、ユーザーが実際に見る pane、buffer、cursor / pointer、editing result などの外部観測結果を含む。
5. 既存の 001 / 002 テスト、workspace tests、fmt、Clippy 等が PASS していても、003 acceptance が FAIL なら 003 は未完了とする。
6. 対象不具合ごとに「どの変更が、どの観測結果を変えたか」を evidence として残す。
7. 対象不具合が一つでも未解決、未再現、未検証なら `003 COMPLETE` を宣言してはならない。

## 3. Primary success condition

003 の最重要成功条件は「検証器が PASS すること」ではなく、ユーザーが以前と同じ操作を行ったとき、以前に壊れていた挙動が実際に変わっていることである。

具体的には、secondary pane を含む pane / buffer / pointer の関係について、操作対象 pane と実際に読み書きされる buffer、表示される cursor / pointer、編集結果が一致し、別 pane の状態に誤って結び付かないことを確認する。

## 4. Non-goals

003 の Goal には以下を含めない。

- 既存テストを単に増やして PASS 数を増やすこと。
- 内部関数の戻り値だけを確認して製品挙動の修正とみなすこと。
- source issue の再現なしに推測ベースで修正を行うこと。
- 003 の対象外リファクタリングを大規模に行うこと。
- oracle / product review の文言だけで完了を代替すること。

## 5. Completion statement

003 の完了報告では、少なくとも次を明記しなければならない。

- Source Issue Matrix の全 ID と最終状態
- 修正前 FAIL の evidence
- 修正後 PASS の evidence
- 実製品 acceptance の実行結果
- 001 / 002 regression の結果
- workspace tests / fmt / Clippy の結果
- 対象コードの変更箇所と、各 source issue への対応関係
- 未解決事項が 0 であること

`tests pass`、`oracle: 0 conflicts`、`product review: 0 blockers` のみでは完了条件を満たさない。
