# AutoQ — Universal Parametric EQ & AutoEQ Suite

**AutoQ** es una suite multiplataforma de ecualización paramétrica (PEQ) y calibración **AutoEQ** con estética inspirada en **Nothing OS** (obsidiana mate, matriz de puntos 5×7 y acento `#D71921`).

Permite controlar tanto **hardware DSP USB-C integrado** (Moondrop Chu 2 DSP, FreeDSP, Rays, Dawn Pro 2, etc.) como **cualquier audífono Bluetooth, DAC USB o salida analógica de 3.5 mm** mediante ecualización global del sistema, manteniendo paridad de funciones entre **Android**, **Linux (Hyprland / Wayland)** y **Linux (KDE Plasma 6)**.

---

## Estructura del monorepo

| Carpeta | Plataforma | Interfaz | Motor de EQ |
| :--- | :--- | :--- | :--- |
| [`android/`](./android/) | **Android 9+ (API 28–36)** | App nativa Jetpack Compose + Widget de pantalla de inicio estilo Nothing OS | USB-C HID (5/8 bandas), Bluetooth SPP y servicio global `GlobalEqService` |
| [`linux-appimage/`](./linux-appimage/) | **Linux Universal (AppImage x86_64)** | App de escritorio independiente (GNOME, KDE, Hyprland, XFCE) + instaladores opcionales de widgets | USB-C HID directo (`/dev/hidraw`), Bluetooth SPP y PipeWire `filter-chain` (10 bandas) |
| [`linux-hyprland/`](./linux-hyprland/) | **Linux / Hyprland (Wayland)** | Widget flotante compacto + App PySide6 de 3 pestañas + módulo Waybar | USB-C HID directo (`/dev/hidraw`), Bluetooth SPP y PipeWire `filter-chain` (10 bandas) |
| [`linux-kde/`](./linux-kde/) | **Linux / KDE Plasma 6** | Plasmoid nativo QML (`org.hector.moondropeq`) + App de escritorio Qt6/PySide6 | Backend JSON CLI compartido (`moondrop_backend.py`) + USB HID / PipeWire / ALSA helper |
| [`windows-beta/`](./windows-beta/) | **Windows 10/11 (`BETA — SIN TESTEAR`)** | App de escritorio PySide6 estilo Nothing OS | USB-C HID vía `hidapi` (KTMicro 5 bandas) + EQ global de 10 bandas vía **Equalizer APO** |
| [`docs/`](./docs/) | **Documentación técnica** | — | [Arquitectura](./docs/ARCHITECTURE.md) y [Protocolo USB HID](./docs/USB_HID_PROTOCOL.md) |

---

## Características principales

* **Control de Hardware DSP USB-C directo (sin app oficial):**
  * **KTMicro 5 bandas (`VID 0x31B2`)**: *Moondrop Chu 2 DSP (`0x0113`)*, *CDSP*, *Quark 2*, etc. Lectura, escritura en vivo (`LIVE`) y guardado en memoria flash (`COMMIT`).
  * **Moondrop 8 bandas**: *FreeDSP Pro / Mini*, *Rays*, *AG Rays*, *Marigold*, *DAWN PRO 2*, *Echo A*, *MOONRIVER 3*, *DHA15*.
* **Ecualizador Universal a nivel de sistema (10 bandas):**
  * En **Linux**, usa `libpipewire-module-filter-chain` nativo en PipeWire para cualquier salida Bluetooth, USB-C o 3.5 mm con latencia mínima y sin procesos pesados.
  * En **Android**, usa `GlobalEqService` para aplicar la curva paramétrica de forma global.
* **Catálogo AutoEQ de +14,500 curvas sincronizado de 8 fuentes abiertas:**
  1. [AutoEq (jaakkopasanen/AutoEq)](https://github.com/jaakkopasanen/AutoEq)
  2. Super\* Review (Squiglink 711)
  3. ReganCipher (Squiglink TWS / 711)
  4. Aftersound (Squiglink 711)
  5. HBB (Squiglink 711)
  6. Paul Wasabii (Squiglink 711)
  7. Jaytiss (Squiglink 711)
  8. GadgetryTech (Squiglink Over-Ear / Headsets)
* **Modo «Imitar cualquier audífono» + 8 curvas objetivo:**
  * Iguala la respuesta en frecuencia de tus audífonos actuales a la de cualquier otro modelo del catálogo (por ejemplo, *Moondrop Chu 2 → 7Hz x Crinacle Zero 3*) o a targets curados (*Harman In-Ear 2019*, *Harman Over-Ear 2018*, *JM-1 con filtros Harman*, *Diffuse Field 5128*, *Oratory1990*).
* **Filtros completos y compatibilidad EqualizerAPO:**
  * Soporte para filtros `PK` (Peaking), `LS` (Low Shelf) y `HS` (High Shelf).
  * Importación y exportación en formato estándar `ParametricEQ.txt` de EqualizerAPO / AutoEq / Squiglink.
* **Bluetooth SPP (opcional):**
  * Lectura/escritura de perfiles (`POP`, `MON`, `BASS`) en modelos Moondrop Bluetooth compatibles.

---

## Inicio rápido por plataforma

### 1. Android (`android/`)
Consulta la guía completa en [`android/README.md`](./android/README.md).
```bash
cd android
./gradlew assembleDebug
```
El APK generado se ubica en `android/app/build/outputs/apk/debug/app-debug.apk`.

### 2. Linux — Hyprland / Wayland (`linux-hyprland/`)
Consulta la guía completa en [`linux-hyprland/README.md`](./linux-hyprland/README.md).
```bash
sudo pacman -S python-pyside6 python-numpy python-requests pipewire wireplumber
cd linux-hyprland
chmod +x install.sh
./install.sh
```
Agrega `source = ~/.local/share/autoq-hyprland/hyprland-autoq.conf` a tu `~/.config/hypr/hyprland.conf` y presiona **`SUPER + E`** para abrir el widget.

### 3. Linux — KDE Plasma 6 (`linux-kde/`)
Consulta la guía completa en [`linux-kde/README.md`](./linux-kde/README.md).
```bash
cd linux-kde
# Instalar el plasmoid de KDE Plasma 6
kpackagetool6 --type Plasma/Applet --install plasmoid
# O ejecutar la app de escritorio Qt6
./launch.sh
```

### 4. Linux Universal — AppImage (`linux-appimage/`)
Consulta la guía completa en [`linux-appimage/README.md`](./linux-appimage/README.md).
```bash
chmod +x AutoQ-x86_64.AppImage
./AutoQ-x86_64.AppImage                        # Abre la app completa independiente
./AutoQ-x86_64.AppImage --install-hyprland-widget   # (Opcional) Instala widget de Hyprland
./AutoQ-x86_64.AppImage --install-kde-plasmoid      # (Opcional) Instala Plasmoid KDE Plasma 6
```

### 5. Windows 10/11 — `BETA / SIN TESTEAR` (`windows-beta/`)
> [!WARNING]
> **Versión Beta sin testear en hardware Windows real.** Consulta las limitaciones y advertencias en [`windows-beta/README.md`](./windows-beta/README.md).

```powershell
cd windows-beta
python -m pip install -r requirements.txt
python autoq_windows.py
```

Los binarios **`AutoQ-x86_64.AppImage`** y **`AutoQ-Windows-Beta-x64.zip`** también se compilan automáticamente mediante **GitHub Actions** ([`.github/workflows/build-releases.yml`](./.github/workflows/build-releases.yml)).

---

## Permisos USB HID en Linux (sin `sudo`)

Para leer y escribir directamente en el chip DSP USB-C (Moondrop Chu 2 DSP, FreeDSP, etc.) desde tu usuario normal, crea `/etc/udev/rules.d/99-autoq-usb-dsp.rules`:

```udev
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="31b2", MODE="0666"
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="2fc6", MODE="0666"
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="2f20", MODE="0666"
```

Luego recarga las reglas:
```bash
sudo udevadm control --reload-rules && sudo udevadm trigger
```

---

## Documentación técnica

* [Arquitectura general y flujo de datos](./docs/ARCHITECTURE.md)
* [Especificación del protocolo USB HID (KTMicro 5-band y Moondrop 8-band)](./docs/USB_HID_PROTOCOL.md)

---

## Licencia

Distribuido bajo licencia **GPL-2.0+** (véase metadatos del plasmoid KDE). Las mediciones de respuesta en frecuencia pertenecen a sus respectivos autores en AutoEq y las bases comunitarias de Squiglink.
