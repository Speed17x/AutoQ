# AutoQ para Windows (`100% Rust Nativo — Beta / Sin Testear`)

> [!WARNING]
> **VERSIÓN BETA / EXPERIMENTAL — SIN TESTEAR EN HARDWARE REAL DE WINDOWS.**  
> Compilada 100 % en **Rust nativo** (`AutoQ.exe`, `autoq-cli.exe` y `autoq-daemon.exe`) usando `hidapi` + Win32, pero **aún no ha sido probada en una máquina Windows física**. Úsala bajo tu propia responsabilidad (especialmente la escritura a Flash USB HID) y reporta cualquier fallo en *Issues*.

---

## Qué incluye esta versión Beta

- **Interfaz Nothing OS completa (`AutoQ.exe`, `eframe`/`egui`)**: 3 pestañas (`EQ`, `AUTOQ`, `AVANZADO`) con gráfica logarítmica interactiva, modo widget compacto, presets e importación/exportación Equalizer APO.
- **USB-C HID y Serial Multiprotocolo (`hidapi` + COM USB-CDC)**:
  - Soporta las 8 familias DSP: **KTMicro 5B**, **Moondrop 8B**, **WalkPlay (5–10B)**, **FiiO / JadeAudio / SNOWSKY (5–10B)**, **Qudelix-5K (10B)**, **Topping DX1 II / E50 II (11B)**, **Fosi Audio DS3 (8B)** y **JDS Labs Element IV (12B USB-CDC)**.
- **EQ Global de 10 bandas vía Equalizer APO**:
  - Genera `autoq_eq.txt` con preamp automático dentro de `C:\Program Files\EqualizerAPO\config\` y agrega `Include: autoq_eq.txt` a `config.txt`.
- **Daemon de auto-perfil opcional (`autoq-daemon.exe`)**.
- **Catálogo AutoEQ de +14,500 curvas** sincronizado de 8 repositorios abiertos.

## Compilar ejecutable `.exe` en Windows (100 % Rust)

```powershell
cd windows-beta
.\build_windows.ps1
```

Genera `dist\AutoQ-Windows\AutoQ.exe`, `autoq-cli.exe` y `autoq-daemon.exe`. También puedes descargar **`AutoQ-Windows-x64.zip`** desde la pestaña **Releases** de GitHub.
