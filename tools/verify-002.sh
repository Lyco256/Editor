#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
pre_goal=0
integrity_only=0
for arg in "$@"; do
  case "$arg" in
    --pre-goal) pre_goal=1 ;;
    --integrity-only) integrity_only=1 ;;
    *) echo "unknown option: $arg" >&2; exit 2 ;;
  esac
done
baseline_path='Requirements/002/BASELINE_REF.txt'
test -f "$baseline_path"
sha="$(tr -d '[:space:]' < "$baseline_path")"
[[ "$sha" =~ ^[0-9a-f]{40}$ ]]
git cat-file -e "${sha}^{commit}"
frozen=(Requirements/002_README.md Requirements/002_GOAL_PROMPT.md Requirements/002_FINAL_GATE.md tools/requirements-002/check_preferred_column_vectors.py tests/requirements_001/navigation.rs tools/verify-002.ps1 tools/verify-002.sh)
while IFS= read -r path; do
  [[ "$path" == *BASELINE_REF.txt ]] || frozen+=("${path#./}")
done < <(find Requirements/002 -type f -print)
git diff --exit-code "$sha" -- "${frozen[@]}"
if (( integrity_only )); then echo '002 baseline integrity: frozen paths are unchanged'; exit 0; fi
python tools/requirements-002/check_preferred_column_vectors.py
for report in docs/testing/requirements-002-review-vector.md docs/testing/requirements-002-review-adapter.md; do test -f "$report"; grep -qx 'CONFLICTS: 0' "$report"; done
test "$(grep -cve '^$' tools/requirements-001/required-cases.txt)" -eq 78
sources="$(find tests/requirements_001 -type f -name '*.rs' -print0 | xargs -0 cat)"
if grep -Eq '#[[:space:]]*\[[[:space:]]*(ignore|cfg)' <<<"$sources"; then echo 'acceptance tests contain ignore/cfg disabling' >&2; exit 1; fi
grep -q 'Requirements/002/vectors/preferred-column.json' tests/requirements_001/navigation.rs
grep -q '"tab-width-4"' Requirements/002/vectors/preferred-column.json
grep -q '"tab-width-8"' Requirements/002/vectors/preferred-column.json
if grep -Fq '12345\\n\\nxyz' tests/requirements_001/navigation.rs || grep -Fq 'CharacterOffset(8)' tests/requirements_001/navigation.rs || grep -Fq 'CharacterOffset(10)' tests/requirements_001/navigation.rs || grep -Fq '\\tabc\\n\\tx' tests/requirements_001/navigation.rs; then exit 1; fi
cargo test --test requirements_001 --no-fail-fast -- --skip k004_preferred_column_empty --skip k005_preferred_column_short --skip k006_preferred_column_tabs
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --test doc_mirror
cargo test --test requirements_001 --no-fail-fast
if (( ! pre_goal )); then test -f docs/testing/requirements-002-final-oracle-review.md; test -f docs/testing/requirements-002-final-product-review.md; grep -qx 'CONFLICTS: 0' docs/testing/requirements-002-final-oracle-review.md; grep -qx 'BLOCKERS: 0' docs/testing/requirements-002-final-product-review.md; fi
echo 'verify-002: all required gates passed'
