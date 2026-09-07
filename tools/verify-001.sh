#!/usr/bin/env bash
set -u
cd "$(dirname "$0")/.."
pre_goal=0
integrity_only=0
if [[ "${1:-}" == "--pre-goal" ]]; then pre_goal=1; fi
if [[ "${1:-}" == "--integrity-only" ]]; then integrity_only=1; fi
manifest=tools/requirements-001/required-cases.txt
mapfile -t cases < <(sed '/^$/d' "$manifest")
[[ "${#cases[@]}" -eq 78 ]] || { echo 'required case manifest must contain 78 IDs' >&2; exit 1; }
mapfile -t source_ids < <(rg -o '`[RKMUPQ][0-9]{3}_[A-Z0-9_]+`' Requirements/001/001_ACCEPTANCE_CASES.md | sed 's/.*`//;s/`.*//' )
for i in "${!cases[@]}"; do [[ "${cases[$i]}" == "${source_ids[$i]}" ]] || { echo 'manifest differs from requirement' >&2; exit 1; }; done
for id in "${cases[@]}"; do rg -qi "fn [a-z0-9_]*${id,,}" tests/requirements_001 || { echo "missing test $id" >&2; exit 1; }; done
! rg -n '#\s*\[\s*ignore|cfg\s*\(' tests/requirements_001 tests/requirements_001.rs >/dev/null || { echo 'acceptance disable found' >&2; exit 1; }
frozen=(Requirements/README.md Requirements/TOP_CODEX_RUNBOOK.md Requirements/AGENT_ORCHESTRATION.md Requirements/GOAL_PROMPT.md Requirements/FINAL_GATE.md tests/requirements_001.rs tools/requirements-001/required-cases.txt tools/verify-001.ps1 tools/verify-001.sh)
while IFS= read -r p; do frozen+=("$p"); done < <(find Requirements/001 -maxdepth 1 -name '*.md' ! -name BASELINE_REF.txt | sort)
while IFS= read -r p; do frozen+=("$p"); done < <(find tests/requirements_001 -type f | sort)
if [[ -f Requirements/001/BASELINE_REF.txt ]]; then
  sha=$(tr -d '\r\n' < Requirements/001/BASELINE_REF.txt)
  [[ "$sha" =~ ^[0-9a-f]{40}$ ]] || { echo 'invalid BASELINE_REF' >&2; exit 1; }
  git cat-file -e "$sha^{commit}" || exit 1
  git diff --exit-code "$sha" -- "${frozen[@]}" || exit 1
elif [[ "$pre_goal" -eq 0 ]]; then echo 'BASELINE_REF required' >&2; exit 1; fi
if [[ "$integrity_only" -eq 1 ]]; then echo 'baseline integrity: frozen paths are unchanged'; exit 0; fi
production=$(find src crates -name '*.rs' -print0 | xargs -0 cat)
for pattern in 'Rect::new\(0,\s*0,\s*120,\s*40\)' 'InputEvent::Resize\s*\{\s*\.\.\s*\}\s*=>\s*Ok\(\(\)\)' 'move_vertical\([^)]*,\s*4\s*\)' 'CharacterOffset\(range.end.0\s*\+\s*1\)' 'line_text\.find\(&result.matched_text\)' 'cell\("▌"'; do
  printf '%s\n' "$production" | rg -n "$pattern" >/dev/null && { echo "forbidden production pattern: $pattern" >&2; exit 1; }
done
cargo fmt --all -- --check || exit 1
cargo clippy --workspace --all-targets --all-features -- -D warnings || exit 1
cargo test --workspace --all-features || exit 1
cargo test --test doc_mirror || exit 1
cargo test --test requirements_001
status=$?
if [[ "$pre_goal" -eq 1 ]]; then [[ "$status" -ne 0 ]] || { echo 'pre-goal suite unexpectedly passed' >&2; exit 1; }; echo "pre-goal acceptance suite is red (exit $status) as required"; exit 0; fi
for report in docs/testing/requirements-001-review-oracle.md docs/testing/requirements-001-review-gate.md docs/testing/requirements-001-review-geometry.md docs/testing/requirements-001-review-bug-sweep.md; do
  [[ -f "$report" ]] && rg -q 'BLOCKERS:[[:space:]]*0' "$report" || { echo "reviewer report missing or blocked: $report" >&2; exit 1; }
done
exit "$status"
