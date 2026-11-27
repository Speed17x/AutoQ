# Trivium — Windows Native Rust Portable & Single-Installer Builder
# Compila Trivium.exe, trivium-cli.exe, trivium-daemon.exe y genera Trivium-Windows-x64.zip + Trivium-Setup-x64.exe

$ErrorActionPreference = "Stop"

Write-Host "============================================================"
Write-Host "  Trivium — Windows Builder (100% Native Rust + Inno Setup)"
Write-Host "============================================================"

Push-Location ..
$CargoVersion = (Select-String -Path "crates\trivium-core\Cargo.toml" -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches.Groups[1].Value
if (-not $CargoVersion) { $CargoVersion = "3.0.0" }
Write-Host "[*] Compilando workspace Rust v$CargoVersion (--release) ..."
cargo build --release --workspace
$Root = Get-Location
Pop-Location

$OutDir = "dist\Trivium-Windows"
if (Test-Path $OutDir) { Remove-Item -Recurse -Force $OutDir }
New-Item -ItemType Directory -Path $OutDir | Out-Null

Copy-Item -Path (Join-Path $Root "target\release\trivium.exe") -Destination (Join-Path $OutDir "Trivium.exe") -Force
Copy-Item -Path (Join-Path $Root "target\release\trivium-cli.exe") -Destination (Join-Path $OutDir "trivium-cli.exe") -Force
Copy-Item -Path (Join-Path $Root "target\release\trivium-daemon.exe") -Destination (Join-Path $OutDir "trivium-daemon.exe") -Force
Copy-Item -Path "README.md" -Destination (Join-Path $OutDir "README.md") -Force

function Sign-IfConfigured {
    param([string]$TargetFile)
    if ($env:WINDOWS_CERT_PFX -and (Test-Path $env:WINDOWS_CERT_PFX) -and $env:WINDOWS_CERT_PASSWORD) {
        $signtool = Get-Command signtool.exe -ErrorAction SilentlyContinue
        if (-not $signtool) {
            $signtool = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin\*\x64\signtool.exe" -ErrorAction SilentlyContinue | Sort-Object FullName -Descending | Select-Object -First 1
        }
        if ($signtool) {
            $toolPath = if ($signtool.Path) { $signtool.Path } else { $signtool.FullName }
            Write-Host "[*] Firmando Authenticode: $TargetFile"
            & $toolPath sign /fd SHA256 /tr "http://timestamp.digicert.com" /td SHA256 /f $env:WINDOWS_CERT_PFX /p $env:WINDOWS_CERT_PASSWORD $TargetFile
            if ($LASTEXITCODE -ne 0) { throw "signtool fallo para $TargetFile" }
        }
    }
}

Sign-IfConfigured (Join-Path $OutDir "Trivium.exe")
Sign-IfConfigured (Join-Path $OutDir "trivium-cli.exe")
Sign-IfConfigured (Join-Path $OutDir "trivium-daemon.exe")

$ZipPath = "dist\Trivium-Windows-x64.zip"
Compress-Archive -Path "$OutDir\*" -DestinationPath $ZipPath -Force
Write-Host "[OK] Portable ZIP generado en: $ZipPath"

$Iscc = Get-Command ISCC.exe -ErrorAction SilentlyContinue
if (-not $Iscc -and (Test-Path "C:\Program Files (x86)\Inno Setup 6\ISCC.exe")) {
    $Iscc = Get-Item "C:\Program Files (x86)\Inno Setup 6\ISCC.exe"
}
if ($Iscc) {
    $IsccPath = if ($Iscc.Path) { $Iscc.Path } else { $Iscc.FullName }
    Write-Host "[*] Compilando instalador único Inno Setup (Trivium-Setup-x64.exe) ..."
    & $IsccPath "/DAppVersion=$CargoVersion" "Trivium.iss"
    if ($LASTEXITCODE -ne 0) { throw "ISCC.exe fallo al generar Trivium-Setup-x64.exe" }
    Sign-IfConfigured "dist\Trivium-Setup-x64.exe"
    Write-Host "[OK] Instalador único generado en: dist\Trivium-Setup-x64.exe"
} else {
    Write-Warning "ISCC.exe (Inno Setup 6) no encontrado; se omitió Trivium-Setup-x64.exe."
}
