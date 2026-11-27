# Trivium para Windows (`100% Rust Nativo`)

> [!NOTE]
> **Probado funcionalmente en Windows 10/11.** `Trivium.exe` es una aplicación GUI pura (`windows_subsystem = "windows"`) que lee USB-C HID vía `hidapi` y puertos COM USB-CDC directamente desde el Registro de Windows (`HKLM\HARDWARE\DEVICEMAP\SERIALCOMM`), **sin abrir ventanas emergentes de PowerShell ni CMD**.
>
> Mientras se completa el alta de firma **Authenticode** gratuita para código abierto (SignPath Foundation), Windows Defender SmartScreen mostrará la advertencia habitual de editor desconocido en binarios nuevos: pulsa **Más información → Ejecutar de todas formas** (y verifica su huella en `SHA256SUMS.txt`).

---

## Opciones de descarga

1. **`Trivium-Setup-x64.exe` (Recomendado):** instalador único generado con Inno Setup 6 (`Trivium.iss`) con accesos directos en Menú Inicio / Escritorio y desinstalador integrado.
2. **`Trivium-Windows-x64.zip`:** versión portable con `Trivium.exe`, `trivium-cli.exe` y `trivium-daemon.exe`.

---

## Qué incluye

- **Interfaz modular (`Trivium.exe`, `eframe`/`egui`)**: temas **`Default`**, **`Default claro`** y **`NothingOS`**, 8 colores de resaltado, gráfica logarítmica interactiva, modo widget compacto, presets e importación/exportación Equalizer APO.
- **USB-C HID y Serial Multiprotocolo (`hidapi` + Registro Win32 COM)**:
  - Soporta las 9 familias DSP: **KTMicro 5B**, **Moondrop 8B**, **Conexant 9B (FreeDSP / ECHO-B)**, **WalkPlay (5–10B)**, **FiiO / JadeAudio / SNOWSKY (5–10B)**, **Qudelix-5K (10B)**, **Topping DX1 II / E50 II (11B)**, **Fosi Audio DS3 (8B)** y **JDS Labs Element IV (12B USB-CDC)**.
- **EQ Global de 10 bandas vía Equalizer APO**:
  - Genera `trivium_eq.txt` con preamp automático y **ganancia/atenuación de salida `-30..+12 dB`** dentro de `C:\Program Files\EqualizerAPO\config\` y agrega `Include: trivium_eq.txt` a `config.txt`.
- **Daemon de auto-perfil opcional (`trivium-daemon.exe`)** (migra perfiles previos de `%LOCALAPPDATA%\AutoQ`).
- **Catálogo AutoEQ de +14,500 curvas** sincronizado de 8 repositorios abiertos.

---

## Compilar en Windows (100 % Rust + Inno Setup)

```powershell
cd windows-beta
.\build_windows.ps1
```

Genera `dist\Trivium-Windows\Trivium.exe`, `dist\Trivium-Windows-x64.zip` y (si Inno Setup 6 / `ISCC.exe` está instalado) **`dist\Trivium-Setup-x64.exe`**. Si se definen `WINDOWS_CERT_PFX` y `WINDOWS_CERT_PASSWORD`, los ejecutables y el instalador se firman automáticamente con `signtool.exe`.
