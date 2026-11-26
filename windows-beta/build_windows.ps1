# AutoQ — Windows Native Rust Portable Builder (ZERO Python / PyInstaller)
# Compila AutoQ.exe (GUI nativa Nothing OS) y autoq-cli.exe en dist/AutoQ-Windows/

$ErrorActionPreference = "Stop"

Write-Host "============================================================"
Write-Host "  AutoQ — Windows Portable Builder (100% Native Rust)"
Write-Host "============================================================"

Push-Location ..
Write-Host "[*] Compilando autoq-gui.exe y autoq-cli.exe en modo release ..."
cargo build --release --workspace
$Root = Get-Location
Pop-Location

$OutDir = "dist\AutoQ-Windows"
if (Test-Path $OutDir) { Remove-Item -Recurse -Force $OutDir }
New-Item -ItemType Directory -Path $OutDir | Out-Null

Copy-Item -Path (Join-Path $Root "target\release\autoq-gui.exe") -Destination (Join-Path $OutDir "AutoQ.exe") -Force
Copy-Item -Path (Join-Path $Root "target\release\autoq-cli.exe") -Destination (Join-Path $OutDir "autoq-cli.exe") -Force
Copy-Item -Path "README.md" -Destination (Join-Path $OutDir "README.md") -Force

Write-Host ""
Write-Host "[OK] Build 100% Rust completado en: $OutDir\AutoQ.exe (~9 MB)"
