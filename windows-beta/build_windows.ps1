# AutoQ — Windows Beta Build Script (Rust Core + PySide6 UI)
# Compila autoq-cli.exe en Rust y genera un ejecutable portable en dist/AutoQ-Windows-Beta/

$ErrorActionPreference = "Stop"

Write-Host "=========================================================="
Write-Host "  AutoQ — Windows BETA Builder (Rust Core + PyInstaller)"
Write-Host "=========================================================="

Push-Location ..
Write-Host "[*] Compilando motor nativo Rust (autoq-cli.exe --release) ..."
cargo build --release -p autoq-cli
$RustCli = Join-Path (Get-Location) "target\release\autoq-cli.exe"
Pop-Location

Copy-Item -Path $RustCli -Destination "backend\autoq-cli.exe" -Force

python -m pip install --upgrade pip
python -m pip install -r requirements.txt pyinstaller

python -m PyInstaller `
    --noconfirm `
    --clean `
    --onedir `
    --windowed `
    --name "AutoQ-Windows-Beta" `
    --paths "." `
    --exclude-module numpy `
    --exclude-module requests `
    --exclude-module PySide6.QtWebEngineCore `
    --exclude-module PySide6.Qt3DCore `
    --exclude-module PySide6.QtMultimedia `
    --exclude-module PySide6.QtQml `
    --exclude-module PySide6.QtQuick `
    --add-binary "$RustCli;." `
    autoq_windows.py

Write-Host ""
Write-Host "[OK] Build completado en: dist\AutoQ-Windows-Beta\AutoQ-Windows-Beta.exe"
