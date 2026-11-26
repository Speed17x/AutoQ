# Changelog — AutoQ

Todos los cambios relevantes de **AutoQ** (Android, Linux Hyprland, Linux KDE Plasma 6, Linux AppImage y Windows Beta) se documentan en este archivo siguiendo [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/) y [Versionado Semántico](https://semver.org/lang/es/).

---

## [1.1.0] — 2026-11-25

### Añadido
- **Motor nativo unificado en Rust (`crates/autoq-core` + `crates/autoq-cli`)**:
  - Binario único de `2.9 MB` (cero dependencias de `numpy`, `requests` u OpenSSL del sistema) con contrato JSON 100 % compatible en Linux y Windows.
  - Optimizador RBJ/PEQ `fit_peq_bands` ×24 más rápido (`18 ms` frente a `439 ms` en Python, con paridad numérica exacta).
  - Driver USB-C HID unificado para **KTMicro 5 bandas** y **Moondrop 8 bandas** en Linux (`/dev/hidraw`) y Windows (`hidapi`), llevando soporte 8-band también a Windows.
  - Subcomandos nativos ultrarrápidos: `autoq-cli poll` (`1 ms`, reemplaza a `moondrop_poll.c`), `autoq-cli waybar-status` y el modo interactivo de terminal `autoq-cli eq` con atribución visible de la fuente de medición (`AutoEq MIT` / `Squiglink`).
  - Migración automática de configuración de `~/.config/chu2-dsp-widget` → `~/.config/autoq`.
- **Integración 1-clic en la pestaña *Avanzado*** (`linux-hyprland/autoq_hyprland.py` y AppImage): botones para instalar **Widget Hyprland**, **Plasmoid KDE 6** y activar **reglas USB `udev`** vía `pkexec`.
- **Empaquetado nativo para Linux (`packaging/` e `install-linux.sh`)**:
  - Arch / CachyOS / EndeavourOS (`packaging/aur/PKGBUILD` y `autoq-bin` para `paru`/`yay`/`pacman`).
  - Debian / Ubuntu / Linux Mint (`packaging/deb/build_deb.sh` generando `autoq_1.1.0_amd64.deb` de `1.5 MB` con reglas `udev` automáticas).
  - Instalador universal en 1 línea `install-linux.sh` (con soporte `--cli-only`).
- **[`ROADMAP.md`](./ROADMAP.md)** con estrategia *Rust-First*.

### Cambiado
- **KDE Plasma 6 (`linux-kde/plasmoid/contents/ui/main.qml`)**: eliminadas rutas absolutas fijas; ahora ejecuta `autoq-cli` desde `$HOME/.local/bin` o `PATH`.
- **AppImage (`linux-appimage/build_appimage.sh`)**: compila y empaqueta `autoq-cli`, genera icono PNG `256x256` (`.DirIcon`) y excluye `numpy`, `requests` y módulos pesados no usados de `PySide6`.
- **Plantilla CI/CD (`docs/github-actions-build-releases.yml`)**: añade `cargo test --workspace`, compilación Rust Linux/Windows, generación de `.deb`, binario `autoq-cli-linux-x86_64`, APK Android y `SHA256SUMS.txt`.

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
