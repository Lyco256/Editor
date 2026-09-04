$ErrorActionPreference = "Stop"

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    $cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
    if (-not (Test-Path (Join-Path $cargoBin "cargo.exe"))) {
        throw "cargo was not found on PATH or in the default rustup location"
    }
    $env:PATH = "$cargoBin;$env:PATH"
}

cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

cargo clippy --workspace --all-targets --all-features -- -D warnings
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

cargo test --workspace --all-features
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

cargo test --test doc_mirror
exit $LASTEXITCODE
