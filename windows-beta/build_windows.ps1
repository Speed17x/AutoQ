# AutoQ — Windows Beta Build Script (PowerShell)
# Compila un ejecutable portable en dist/AutoQ-Windows-Beta/

$ErrorActionPreference = "Stop"

Write-Host "=============================================="
Write-Host "  AutoQ — Windows BETA Builder (PyInstaller)"
Write-Host "=============================================="

python -m pip install --upgrade pip
python -m pip install -r requirements.txt pyinstaller

python -m PyInstaller `
    --noconfirm `
    --clean `
    --onedir `
    --windowed `
    --name "AutoQ-Windows-Beta" `
    --paths "." `
    --paths "backend" `
    --hidden-import moondrop_backend_win `
    --hidden-import chu2_hid_win `
    --hidden-import win_eq_apo `
    --hidden-import autoeq_db `
    --hidden-import biquad `
    --hidden-import hid `
    --add-data "backend;backend" `
    autoq_windows.py

Write-Host ""
Write-Host "[OK] Build completado en: dist\AutoQ-Windows-Beta\AutoQ-Windows-Beta.exe"
