# Builds Avix in release mode and copies the result to .\dist\Avix.exe
# Usage:  powershell -ExecutionPolicy Bypass -File .\build.ps1
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    $env:Path = (Join-Path $env:USERPROFILE '.cargo\bin') + ";$env:Path"
}

# --locked keeps the dependency pins in Cargo.lock (needed for the GNU toolchain, see README).
cargo build --release --locked
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

New-Item -ItemType Directory -Force dist | Out-Null
Copy-Item target\release\avix.exe dist\Avix.exe -Force
"Built dist\Avix.exe ({0:N1} MB)" -f ((Get-Item dist\Avix.exe).Length / 1MB)
