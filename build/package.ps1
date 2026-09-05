param(
    [string]$Version = "0.1.0",
    [string]$Configuration = "release"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$target = Join-Path $root "target\$Configuration\editor.exe"
if (-not (Test-Path -LiteralPath $target)) {
    & cargo build --manifest-path (Join-Path $root "Cargo.toml") --release
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
}

$staging = Join-Path $root "target\package\Editor-$Version"
if (Test-Path -LiteralPath $staging) { Remove-Item -LiteralPath $staging -Recurse -Force }
New-Item -ItemType Directory -Path $staging | Out-Null
Copy-Item -LiteralPath $target -Destination (Join-Path $staging "editor.exe")
Copy-Item -LiteralPath (Join-Path $root "LICENSE") -Destination $staging -ErrorAction SilentlyContinue

$archive = Join-Path $root "target\package\Editor-$Version-windows-x64.zip"
if (Test-Path -LiteralPath $archive) { Remove-Item -LiteralPath $archive -Force }
Compress-Archive -LiteralPath $staging -DestinationPath $archive
Write-Output $archive
