#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
final=0
for arg in "$@"; do
  case "$arg" in
    --final) final=1 ;;
    *) echo "unknown option: $arg" >&2; exit 2 ;;
  esac
done
head_sha="$(git rev-parse HEAD)"
if [[ -n "$(git status --porcelain)" ]]; then state=DIRTY; else state=CLEAN; fi
echo "TARGET_COMMIT: $head_sha"
echo "WORKTREE: $state"
if (( final )) && [[ "$state" != CLEAN ]]; then echo 'final mode requires a clean worktree' >&2; exit 1; fi
matrix=docs/testing/requirements-003-evidence-matrix.md
test -f "$matrix"
for id in S003-01 S003-02 S003-03; do grep -q "^- SOURCE_ISSUE_ID: $id$" "$matrix"; done
for field in USER_VISIBLE_SYMPTOM MINIMAL_REPRODUCTION EXPECTED_RESULT BASELINE_ACTUAL_RESULT BASELINE_EVIDENCE FIX_COMMIT POST_FIX_ACTUAL_RESULT POST_FIX_EVIDENCE REGRESSION_TEST_ID FINAL_STATUS; do grep -Eq "^- $field:[[:space:]]+[^[:space:]].*" "$matrix"; done
test "$(grep -c '^- FINAL_STATUS: CLOSED$' "$matrix")" -eq 3
if grep -Eq '^- FINAL_STATUS: (OPEN|UNREPRODUCED|FIXED_UNVERIFIED)$' "$matrix"; then exit 1; fi
baseline=docs/testing/requirements-003-baseline-red.log
grep -qx 'BASELINE_COMMIT: 145b10c7e743726ed38fbef02b04c56d530c249a' "$baseline"
grep -qx 'EXIT_CODE: 101' "$baseline"
for id in S003-01 S003-02 S003-03; do grep -q "^$id: FAILED" "$baseline"; done
git cat-file -e '145b10c7e743726ed38fbef02b04c56d530c249a^{commit}'
green=docs/testing/requirements-003-postfix-green.log
grep -qx 'FIX_COMMIT: bb94d29ac03b3e8b125b15c41afd8122661644ca' "$green"
grep -qx 'EXIT_CODE: 0' "$green"
for id in S003-01 S003-02 S003-03; do grep -q "^$id: PASS" "$green"; done
grep -qx 'MANUAL PARITY: PASS' docs/testing/requirements-003-manual-parity.log
if grep -Eq '#[[:space:]]*\[[[:space:]]*(ignore|cfg)|skip|xfail' tests/requirements_003.rs; then echo 'disabled acceptance case' >&2; exit 1; fi
cargo test --test requirements_003 --no-fail-fast -- --nocapture
echo 'PRODUCT ACCEPTANCE: PASS'
cargo test --test requirements_001 --no-fail-fast
echo '001 REGRESSION: PASS'
pwsh -NoProfile -File tools/verify-002.ps1 -PreGoal
echo '002 REGRESSION: PASS'
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --test doc_mirror
if (( final )) && [[ -n "$(git status --porcelain)" ]]; then echo 'final mode ended dirty' >&2; exit 1; fi
echo 'REQUIREMENTS 003: PASS'
echo 'SOURCE ISSUES: 3/3 CLOSED'
echo 'BASELINE RED EVIDENCE: 3/3'
echo 'PRODUCT ACCEPTANCE: PASS'
echo '001 REGRESSION: PASS'
echo '002 REGRESSION: PASS'
echo 'WORKSPACE TESTS: PASS'
echo 'FMT/LINT: PASS'
echo 'EVIDENCE CROSS-CHECK: PASS'
echo 'WORKTREE: CLEAN'
