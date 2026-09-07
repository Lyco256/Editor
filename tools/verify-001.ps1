[CmdletBinding()]
param([switch]$PreGoal, [switch]$IntegrityOnly)
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

$frozen = @(
  'Requirements/README.md','Requirements/TOP_CODEX_RUNBOOK.md','Requirements/AGENT_ORCHESTRATION.md',
  'Requirements/GOAL_PROMPT.md','Requirements/FINAL_GATE.md','tests/requirements_001.rs',
  'tools/requirements-001/required-cases.txt','tools/verify-001.ps1','tools/verify-001.sh'
)
$frozen += (Get-ChildItem Requirements/001 -Filter *.md | Sort-Object FullName | ForEach-Object { $_.FullName.Substring((Get-Location).Path.Length + 1).Replace('\','/') })
$frozen += (Get-ChildItem tests/requirements_001 -Recurse -File | ForEach-Object { $_.FullName.Substring((Get-Location).Path.Length + 1).Replace('\','/') })
$manifest = @(Get-Content tools/requirements-001/required-cases.txt | Where-Object { $_ -ne '' })
if ($manifest.Count -ne 78) { throw "required case manifest must contain 78 IDs" }
$ids = @(Select-String Requirements/001/001_ACCEPTANCE_CASES.md -Pattern '`([RKMUPQ][0-9]{3}_[A-Z0-9_]+)`' -AllMatches | ForEach-Object { $_.Matches } | ForEach-Object { $_.Groups[1].Value })
if (($ids -join "`n") -cne ($manifest -join "`n")) { throw 'required case manifest differs from 001 acceptance cases' }
$sources = (Get-ChildItem tests/requirements_001 -Recurse -Filter *.rs | ForEach-Object { Get-Content $_.FullName -Raw }) -join "`n"
foreach ($id in $manifest) { if ($sources -notmatch "(?m)fn\s+[a-z0-9_]*$([regex]::Escape($id.ToLower()))") { throw "missing acceptance test name for $id" } }
if ($sources -match '#\s*\[\s*ignore|cfg\s*\(') { throw 'acceptance tests contain ignore/cfg disabling' }

$refPath = 'Requirements/001/BASELINE_REF.txt'
if (Test-Path $refPath) {
  $sha = (Get-Content $refPath -Raw).Trim()
  if ($sha -notmatch '^[0-9a-f]{40}$') { throw 'BASELINE_REF must contain one 40-hex SHA' }
  git cat-file -e "$sha^{commit}"
  git diff --exit-code $sha -- $frozen | Out-Null
} elseif (-not $PreGoal) { throw 'BASELINE_REF.txt is required outside --pre-goal setup' }
if ($IntegrityOnly) { Write-Host 'baseline integrity: frozen paths are unchanged'; exit 0 }

if (-not (Test-Path Requirements/001/075_FINAL_VERIFIER_AND_FORBIDDEN_PATH_GUARD.md)) { throw '075 guard requirement missing' }
$production = @('src','crates') | ForEach-Object { Get-ChildItem $_ -Recurse -Filter *.rs | Get-Content -Raw } | Out-String
@('Rect::new\(0,\s*0,\s*120,\s*40\)','InputEvent::Resize\s*\{\s*\.\.\s*\}\s*=>\s*Ok\(\(\)\)','move_vertical\([^\)]*,\s*4\s*\)','CharacterOffset\(range\.end\.0\s*\+\s*1\)','column\s*>=\s*60|row\s*>=\s*20','line_text\.find\(&result\.matched_text\)','cell\("▌"').ForEach({ if ($production -match $_) { throw "forbidden production pattern: $_" } })

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --test doc_mirror
$acceptance = 0
cargo test --test requirements_001
$acceptance = $LASTEXITCODE
if ($PreGoal) {
  if ($acceptance -eq 0) { throw 'pre-goal acceptance suite unexpectedly passed' }
  Write-Host "pre-goal acceptance suite is red (exit $acceptance) as required"
} elseif ($acceptance -ne 0) { exit $acceptance }
if (-not $PreGoal) {
  $reports = @('docs/testing/requirements-001-review-oracle.md','docs/testing/requirements-001-review-gate.md','docs/testing/requirements-001-review-geometry.md','docs/testing/requirements-001-review-bug-sweep.md')
  foreach ($report in $reports) { if (-not (Test-Path $report) -or -not (Select-String -Path $report -Pattern 'BLOCKERS:\s*0')) { throw "reviewer report missing or blocked: $report" } }
}
exit 0
