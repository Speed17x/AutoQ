# Distribución en Windows: WinGet y Firma Authenticode (SignPath)

## 1. Publicar en `microsoft/winget-pkgs`

Una vez publicado un release en GitHub que incluya `Trivium-Setup-x64.exe` y `SHA256SUMS.txt`:

1. Copia el hash SHA-256 de `Trivium-Setup-x64.exe` desde `SHA256SUMS.txt`.
2. Actualiza `PackageVersion`, `InstallerUrl` e `InstallerSha256` en [`Speed17x.Trivium.yaml`](./Speed17x.Trivium.yaml).
3. Valida el manifiesto con la herramienta oficial `wingetcreate` o `winget validate --manifest packaging/winget/Speed17x.Trivium.yaml` en Windows 10/11.
4. Abre un Pull Request hacia `https://github.com/microsoft/winget-pkgs` (en `manifests/s/Speed17x/Trivium/<version>/`) para habilitar:
   ```powershell
   winget install Speed17x.Trivium
   ```

## 2. Firma Authenticode gratuita para proyectos GPL (SignPath Foundation)

1. Crea una cuenta y solicita registro del proyecto open-source bajo licencia `GPL-2.0-or-later` (`LICENSE` en la raíz del repo) en https://signpath.org/projects.
2. Conecta el workflow [`.github/workflows/build-releases.yml`](../../.github/workflows/build-releases.yml) usando una política de firma que tome `Trivium.exe`, `trivium-cli.exe`, `trivium-daemon.exe` y `Trivium-Setup-x64.exe` antes de publicar el release, o configura los secretos opcionales `WINDOWS_CERT_BASE64` y `WINDOWS_CERT_PASSWORD` ya soportados por el workflow.
