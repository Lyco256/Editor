# Requirements 003 — Implementation Constraints

## 1. Root-cause first

修正は「secondary pane だから分岐を追加する」のような symptom patch を第一選択にしない。

pane / buffer / cursor / pointer の ownership と resolution path を追跡し、どの境界で誤った identity / binding が混入するかを特定する。

## 2. Traceability

対象操作について、debug/test instrumentation で以下の resolution chain を追跡可能にする。

```text
input event
→ target/focused pane
→ pane-local view state
→ displayed buffer/document
→ operation target resolution
→ cursor/pointer owner
→ mutation target
→ rendered/observable result
```

S003-02 の baseline ではこの chain のどこで expected と diverge するかを記録する。

## 3. Ownership invariants

実装は少なくとも以下の invariant を明確にする。

- Pane identity と Buffer identity は別物。
- 同一 Buffer を複数 Pane が表示できる場合、buffer-shared state と pane-local state を区別する。
- pointer / cursor が pane-local なら、その owner を buffer だけから逆引きしない。
- focused pane と mutation target の解決が別経路なら、最終的な整合条件を持つ。
- stale pane/buffer binding を focus switch や buffer switch 後に再利用しない。

## 4. Regression test placement

根本原因を修正した層には unit / integration regression を追加してよいが、それだけでは 003 acceptance を満たさない。

必要なテスト層は次の 2 層以上とする。

1. root-cause 層の regression test
2. product-boundary の source-issue acceptance

## 5. Scope control

003 の修正中に別の defect を発見した場合:

- 同じ root cause で 003 acceptance に影響するなら 003 に含める。
- 無関係なら別 issue として記録し、003 の完了条件を曖昧化しない。

## 6. No behaviorless refactor credit

コード構造の改善、resolver の一般化、pointer separation abstraction の導入自体は成果とみなさない。

同じ source reproduction で observable result が変わったことを証明して初めて修正として認める。
