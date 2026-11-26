# Changelog — AutoQ

Todos los cambios relevantes de **AutoQ** (Android, Linux Hyprland, Linux KDE Plasma 6, Linux AppImage y Windows Beta) se documentan en este archivo siguiendo [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/) y [Versionado Semántico](https://semver.org/lang/es/).

---

## [1.0.0] — 2026-11-25

### Añadido
- **Monorepo unificado (`Speed17x/AutoQ`)** con paridad de funciones entre plataformas:
  - **Android (`android/`)**: app nativa en Kotlin + Jetpack Compose estilo Nothing OS, widget de pantalla de inicio (`AutoQ · Nothing`), transporte USB-C HID (KTMicro 5 bandas / Moondrop 8 bandas), Bluetooth SPP y `GlobalEqService`.
  - **Linux Hyprland (`linux-hyprland/`)**: widget flotante compacto + app completa PySide6 de 3 pestañas, IPC por socket Unix (`--toggle` para `SUPER+E`) y salida JSON para Waybar (`--status-waybar`).
  - **Linux KDE Plasma 6 (`linux-kde/`)**: Plasmoid nativo QML (`org.hector.moondropeq`), app independiente Qt6 (`widget_app.py`), sondeo rápido `moondrop_poll.c` y helper ALSA `moondrop_unity_pcm.sh`.
  - **Linux AppImage (`linux-appimage/`)**: empaquetado portable `x86_64` para cualquier distribución/escritorio Linux, con instaladores opcionales integrados (`--install-hyprland-widget`, `--install-kde-plasmoid`).
  - **Windows Beta (`windows-beta/`)** *(experimental, sin testear en hardware Windows real)*: app PySide6 estilo Nothing OS con transporte USB-C HID vía `hidapi` (KTMicro 5 bandas) y EQ global de 10 bandas vía **Equalizer APO**.
- **Motor AutoEQ de 8 fuentes abiertas (+14,500 curvas)**: AutoEq GitHub + 7 bases Squiglink (*Super\* Review, ReganCipher, Aftersound, HBB, Paul Wasabii, Jaytiss, GadgetryTech*), 8 curvas objetivo curadas y modo «Imitar cualquier audífono».
- **Documentación técnica completa**:
  - `README.md` principal y READMEs dedicados por plataforma.
  - `docs/ARCHITECTURE.md` (arquitectura y flujo de datos).
  - `docs/USB_HID_PROTOCOL.md` (especificación binaria KTMicro 5-band y Moondrop 8-band + reglas `udev`).
  - `docs/github-actions-build-releases.yml` (plantilla de CI/CD para compilar `.AppImage` y `.exe` en GitHub Actions).
