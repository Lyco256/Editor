[CmdletBinding()]
param([switch]$Final)
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

function Require-Text([string]$Path, [string]$Pattern, [string]$Message) {
  if (-not (Test-Path $Path)) { throw "$Message (missing $Path)" }
  if (-not (Select-String -Path $Path -Pattern $Pattern -Quiet)) { throw $Message }
}
function Invoke-Required([string]$File, [string[]]$Arguments) {
  & $File @Arguments
  if ($LASTEXITCODE -ne 0) { throw "$File $($Arguments -join ' ') failed with exit $LASTEXITCODE" }
}

$head = (git rev-parse HEAD).Trim()
$status = @(git status --porcelain)
Write-Host "TARGET_COMMIT: $head"
Write-Host "WORKTREE: $(if ($status.Count -eq 0) { 'CLEAN' } else { 'DIRTY' })"
if ($Final -and $status.Count -ne 0) { throw 'final mode requires a clean worktree' }

$matrix = 'docs/testing/requirements-003-evidence-matrix.md'
Require-Text $matrix '^# Requirements 003 source-issue evidence matrix' 'evidence matrix missing'
foreach ($id in @('S003-01','S003-02','S003-03')) {
  Require-Text $matrix "(?m)^- SOURCE_ISSUE_ID: $id$" "matrix missing $id"
  $block = [regex]::Match((Get-Content $matrix -Raw), "(?ms)^## $id\r?\n(.*?)(?=^## S003-|\z)").Groups[1].Value
  foreach ($field in @('USER_VISIBLE_SYMPTOM','MINIMAL_REPRODUCTION','EXPECTED_RESULT','BASELINE_ACTUAL_RESULT','BASELINE_EVIDENCE','FIX_COMMIT','POST_FIX_ACTUAL_RESULT','POST_FIX_EVIDENCE','REGRESSION_TEST_ID','FINAL_STATUS')) {
    if ($block -notmatch "(?m)^- ${field}:\s*\S") { throw "$id has blank required field $field" }
  }
  if ($block -notmatch '(?m)^- FINAL_STATUS: CLOSED$') { throw "$id is not CLOSED" }
}
if ((Get-Content $matrix -Raw) -match '(?m)^- FINAL_STATUS: (OPEN|UNREPRODUCED|FIXED_UNVERIFIED)$') { throw 'matrix contains unresolved source issue' }

$baseline = 'docs/testing/requirements-003-baseline-red.log'
Require-Text $baseline '^BASELINE_COMMIT: 145b10c7e743726ed38fbef02b04c56d530c249a$' 'baseline SHA mismatch'
Require-Text $baseline '^EXIT_CODE: 101$' 'baseline did not fail closed'
foreach ($id in @('S003-01','S003-02','S003-03')) { Require-Text $baseline "^${id}: FAILED" "baseline FAIL evidence missing for $id" }
git cat-file -e '145b10c7e743726ed38fbef02b04c56d530c249a^{commit}'
if ($LASTEXITCODE -ne 0) { throw 'baseline commit is unavailable' }

$green = 'docs/testing/requirements-003-postfix-green.log'
Require-Text $green '^FIX_COMMIT: bb94d29ac03b3e8b125b15c41afd8122661644ca$' 'fix SHA mismatch'
Require-Text $green '^EXIT_CODE: 0$' 'post-fix acceptance did not pass'
foreach ($id in @('S003-01','S003-02','S003-03')) { Require-Text $green "^${id}: PASS" "post-fix PASS evidence missing for $id" }
Require-Text 'docs/testing/requirements-003-manual-parity.log' '^MANUAL PARITY: PASS$' 'manual parity evidence missing'

Invoke-Required 'cargo' @('test','--test','requirements_003','--no-fail-fast','--','--nocapture')
Write-Host 'PRODUCT ACCEPTANCE: PASS'
Invoke-Required 'cargo' @('test','--test','requirements_001','--no-fail-fast')
Write-Host '001 REGRESSION: PASS'
Invoke-Required 'pwsh' @('-NoProfile','-File','tools/verify-002.ps1','-PreGoal')
Write-Host '002 REGRESSION: PASS'
Invoke-Required 'cargo' @('fmt','--all','--','--check')
Invoke-Required 'cargo' @('clippy','--workspace','--all-targets','--all-features','--','-D','warnings')
Invoke-Required 'cargo' @('test','--workspace','--all-features')
Invoke-Required 'cargo' @('test','--test','doc_mirror')
if (Select-String -Path 'tests/requirements_003.rs' -Pattern '#\s*\[\s*(ignore|cfg)|skip|xfail' -Quiet) { throw '003 acceptance contains a disabled/waived case' }

if ($Final) {
  $remaining = @(git status --porcelain)
  if ($remaining.Count -ne 0) { throw 'final mode ended with a dirty worktree' }
}
Write-Host 'REQUIREMENTS 003: PASS'
Write-Host 'SOURCE ISSUES: 3/3 CLOSED'
Write-Host 'BASELINE RED EVIDENCE: 3/3'
Write-Host 'PRODUCT ACCEPTANCE: PASS'
Write-Host '001 REGRESSION: PASS'
Write-Host '002 REGRESSION: PASS'
Write-Host 'WORKSPACE TESTS: PASS'
Write-Host 'FMT/LINT: PASS'
Write-Host 'EVIDENCE CROSS-CHECK: PASS'
Write-Host 'WORKTREE: CLEAN'
