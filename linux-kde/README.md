# AutoQ para KDE Plasma 6 (Plasmoid + App Qt6)

Implementación nativa para **KDE Plasma 6** compuesta por un **Plasmoid QML** (`org.hector.moondropeq`), una **aplicación de escritorio PySide6** (`widget_app.py`) y el backend CLI en Python (`moondrop_backend.py`) compartido con la versión de Hyprland.

## Componentes incluidos

| Archivo / Carpeta | Descripción |
| :--- | :--- |
| `plasmoid/` | Applet nativo de KDE Plasma 6 (`org.hector.moondropeq`, API Plasma 6.0) escrito en QML. |
| `widget_app.py` | Aplicación de escritorio Qt6 / PySide6 con gráfica interactiva y gestor de presets. |
| `launch.sh` | Script lanzador para `widget_app.py`. |
| `moondrop_backend.py` | Backend CLI JSON invocado por el plasmoid para HID USB, PipeWire PEQ y catálogo AutoEq. |
| `chu2_hid.py` | Transporte directo `/dev/hidraw` para KTMicro 5-band y Moondrop 8-band. |
| `moondrop_bt_pw.py` | Integración con PipeWire (`filter-chain` de 10 bandas) y Bluetooth SPP. |
| `autoeq_db.py` | Descarga paralela, caché local y emparejamiento de curvas desde 8 repositorios abiertos. |
| `biquad.py` | Motor matemático de filtros biquad y optimización de bandas PEQ. |
| `moondrop_poll.c` | Utilidad ligera en C para sondeo rápido de estado del dispositivo. |
| `moondrop_unity_pcm.sh` | Script auxiliar ALSA para fijar el mezclador hardware `PCM` al 100% (0 dB) en DACs `31b2:*` cuando se usa `soft-mixer` en WirePlumber. |

## Dependencias (Arch / CachyOS)
```bash
sudo pacman -S python-pyside6 python-numpy python-requests pipewire wireplumber
```

## Instalación

### 1. Instalar o actualizar el Plasmoid de KDE Plasma 6
```bash
cd linux-kde
kpackagetool6 --type Plasma/Applet --install plasmoid
# Si ya estaba instalado y quieres actualizarlo:
kpackagetool6 --type Plasma/Applet --upgrade plasmoid
```
Luego agrégalo a tu panel o escritorio buscando **AutoQ** en *Añadir elementos gráficos*.

### 2. Ejecutar la aplicación independiente Qt6
```bash
cd linux-kde
./launch.sh
```

### 3. Compilar `moondrop_poll` (opcional)
```bash
cd linux-kde
gcc -O2 -Wall -o moondrop_poll moondrop_poll.c
```

## Configuración y caché
- Presets de usuario: `~/.config/chu2-dsp-widget/presets.json`
- Caché de curvas AutoEq / Squiglink: `~/.cache/chu2-dsp-widget/`
