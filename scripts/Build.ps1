$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$cargo = Join-Path $HOME '.cargo\bin\cargo.exe'
if (-not (Test-Path -LiteralPath $cargo)) { $cargo = (Get-Command cargo -ErrorAction Stop).Source }
Push-Location $root
try {
    & $cargo fmt --check
    if ($LASTEXITCODE -ne 0) { throw 'Formatting check failed.' }
    & $cargo clippy --locked --all-targets -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw 'Clippy failed.' }
    & $cargo test --locked
    if ($LASTEXITCODE -ne 0) { throw 'Tests failed.' }
    & $cargo build --release --locked
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed.' }
    New-Item -ItemType Directory -Force -Path (Join-Path $root 'dist') | Out-Null
    Copy-Item -LiteralPath (Join-Path $root 'target\release\quiet-rgb.exe') -Destination (Join-Path $root 'dist\QuietRGB.exe') -Force
    foreach ($file in @('README.md','LICENSE','THIRD_PARTY.md')) {
        Copy-Item -LiteralPath (Join-Path $root $file) -Destination (Join-Path $root "dist\$file") -Force
    }
    Write-Output 'Built dist\QuietRGB.exe'
}
finally { Pop-Location }
