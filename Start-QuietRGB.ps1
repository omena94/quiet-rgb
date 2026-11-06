$ErrorActionPreference = 'Stop'
$app = Join-Path $PSScriptRoot 'dist\QuietRGB.exe'
if (-not (Test-Path -LiteralPath $app)) {
    throw 'Run scripts\Build.ps1 first to build QuietRGB.exe.'
}
$existing = Get-Process QuietRGB -ErrorAction SilentlyContinue |
    Where-Object { $_.Path -eq $app }
if (-not $existing) { Start-Process -FilePath $app -WorkingDirectory $PSScriptRoot }
