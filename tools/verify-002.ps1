[CmdletBinding()]
param([switch]$PreGoal, [switch]$IntegrityOnly)
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

function Invoke-Required([string]$File, [string[]]$Arguments) {
  & $File @Arguments
  if ($LASTEXITCODE -ne 0) { throw "$File $($Arguments -join ' ') failed with exit $LASTEXITCODE" }
}

$baselinePath = 'Requirements/002/BASELINE_REF.txt'
if (-not (Test-Path $baselinePath)) { throw 'Requirements/002/BASELINE_REF.txt is required' }
$sha = (Get-Content $baselinePath -Raw).Trim()
if ($sha -notmatch '^[0-9a-f]{40}$') { throw '002 BASELINE_REF must contain one 40-hex SHA' }
git cat-file -e "$sha^{commit}"
if ($LASTEXITCODE -ne 0) { throw '002 baseline commit does not exist' }

$frozen = @(
  'Requirements/002_README.md', 'Requirements/002_GOAL_PROMPT.md', 'Requirements/002_FINAL_GATE.md',
  'tools/requirements-002/check_preferred_column_vectors.py',
  'tests/requirements_001/navigation.rs', 'tools/verify-002.ps1', 'tools/verify-002.sh'
)
$frozen += Get-ChildItem Requirements/002 -Recurse -File |
  Where-Object { $_.Name -ne 'BASELINE_REF.txt' } |
  ForEach-Object { $_.FullName.Substring((Get-Location).Path.Length + 1).Replace('\','/') }
git diff --exit-code $sha -- $frozen | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'frozen 002 paths differ from BASELINE_REF' }
if ($IntegrityOnly) { Write-Host '002 baseline integrity: frozen paths are unchanged'; exit 0 }

Invoke-Required 'python' @('tools/requirements-002/check_preferred_column_vectors.py')
foreach ($report in @('docs/testing/requirements-002-review-vector.md','docs/testing/requirements-002-review-adapter.md')) {
  if (-not (Test-Path $report)) { throw "pre-freeze reviewer report missing: $report" }
  if (-not (Select-String -Path $report -Pattern '^CONFLICTS: 0$')) { throw "reviewer report is not zero-conflict: $report" }
}

$manifest = @(Get-Content tools/requirements-001/required-cases.txt | Where-Object { $_ -ne '' })
if ($manifest.Count -ne 78) { throw 'required case manifest must contain 78 IDs' }
$sources = (Get-ChildItem tests/requirements_001 -Recurse -Filter *.rs | Get-Content -Raw) -join "`n"
foreach ($id in $manifest) {
  $name = $id.ToLowerInvariant()
  if ($sources -notmatch "(?m)fn\s+[a-z0-9_]*$([regex]::Escape($name))\s*\(") { throw "missing acceptance test name for $id" }
}
if ($sources -match '#\s*\[\s*ignore|cfg\s*\(') { throw 'acceptance tests contain ignore/cfg disabling' }

$adapter = Get-Content tests/requirements_001/navigation.rs -Raw
if ($adapter -notmatch 'Requirements/002/vectors/preferred-column\.json') { throw 'K004-K006 adapter does not load canonical vectors' }
$vectors = Get-Content Requirements/002/vectors/preferred-column.json -Raw | ConvertFrom-Json
$k006 = @($vectors.vectors | Where-Object { $_.case_id -eq 'K006_PREFERRED_COLUMN_TABS' } | ForEach-Object { $_.variant_id })
if (-not ($k006 -contains 'tab-width-4') -or -not ($k006 -contains 'tab-width-8')) { throw 'both K006 canonical variants are required' }
foreach ($old in @('12345\n\nxyz','CharacterOffset(8)','CharacterOffset(10)','\\tabc\n\\tx')) {
  if ($adapter -match [regex]::Escape($old)) { throw "old invalid preferred-column fixture remains: $old" }
}

Invoke-Required 'cargo' @('test','--test','requirements_001','--no-fail-fast','--','--skip','k004_preferred_column_empty','--skip','k005_preferred_column_short','--skip','k006_preferred_column_tabs')
Invoke-Required 'cargo' @('fmt','--all','--','--check')
Invoke-Required 'cargo' @('clippy','--workspace','--all-targets','--all-features','--','-D','warnings')
Invoke-Required 'cargo' @('test','--workspace','--all-features')
Invoke-Required 'cargo' @('test','--test','doc_mirror')
Invoke-Required 'cargo' @('test','--test','requirements_001','--no-fail-fast')

if (-not $PreGoal) {
  if (-not (Test-Path 'docs/testing/requirements-002-final-oracle-review.md')) { throw 'final oracle reviewer report missing' }
  if (-not (Test-Path 'docs/testing/requirements-002-final-product-review.md')) { throw 'final product reviewer report missing' }
  if (-not (Select-String -Path 'docs/testing/requirements-002-final-oracle-review.md' -Pattern '^CONFLICTS: 0$')) { throw 'final oracle review is not conflict-free' }
  if (-not (Select-String -Path 'docs/testing/requirements-002-final-product-review.md' -Pattern '^BLOCKERS: 0$')) { throw 'final product review is not blocker-free' }
}
Write-Host 'verify-002: all required gates passed'
