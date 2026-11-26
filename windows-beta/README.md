# AutoQ para Windows (`BETA — SIN TESTEAR`)

> [!WARNING]
> **VERSIÓN BETA / EXPERIMENTAL — SIN TESTEAR EN HARDWARE REAL DE WINDOWS.**  
> Esta adaptación fue desarrollada a partir del núcleo probado en Linux/Android, pero **aún no ha sido probada en una máquina Windows física**. Úsala bajo tu propia responsabilidad (especialmente la escritura a Flash USB HID) y reporta cualquier fallo en *Issues*.

---

## Qué incluye esta versión Beta

- **Interfaz Nothing OS completa (PySide6)**: las mismas 3 pestañas (`EQ`, `AUTOQ`, `AVANZADO`) con gráfica interactiva, presets e importación/exportación EqualizerAPO.
- **USB-C HID directo vía `hidapi`** (`backend/chu2_hid_win.py`):
  - Implementa el protocolo binario KTMicro de **5 bandas** (`VID 0x31B2`: Moondrop Chu 2 DSP, CDSP, Quark 2).
  - *Nota:* el protocolo Moondrop de 8 bandas aún requiere validación en Windows.
- **EQ Global de 10 bandas vía Equalizer APO** (`backend/win_eq_apo.py`):
  - Para **cualquier audífono Bluetooth, 3.5 mm o DAC USB** en Windows.
  - Genera `autoq_eq.txt` con preamp automático dentro de `C:\Program Files\EqualizerAPO\config\` y agrega `Include: autoq_eq.txt` a `config.txt`.
- **Catálogo AutoEQ de +14,500 curvas** sincronizado de 8 repositorios abiertos (`autoeq_db.py`).

## Limitaciones conocidas (Beta)
- No ha sido testeada en Windows 10/11 real.
- Bluetooth SPP (`POP/MON/BASS`) aún no está implementado en Windows.
- Para escribir en `C:\Program Files\EqualizerAPO\config\`, puede ser necesario ejecutar AutoQ como **Administrador** (o dar permisos de escritura a esa carpeta).

---

## Instalación y ejecución desde código fuente

1. Instala **Python 3.10+** y (opcionalmente, para EQ global de 10 bandas) **[Equalizer APO](https://sourceforge.net/projects/equalizerapo/)**.
2. Instala las dependencias:
   ```powershell
   cd windows-beta
   python -m pip install -r requirements.txt
   ```
3. Ejecuta AutoQ:
   ```powershell
   python autoq_windows.py
   ```

## Compilar ejecutable `.exe` (PyInstaller)

```powershell
cd windows-beta
.\build_windows.ps1
```

También puedes descargar la compilación automática desde la pestaña **Actions** / **Releases** de GitHub (`.github/workflows/build-releases.yml`).
