# Changelog — AutoQ

Todos los cambios relevantes de **AutoQ** (Android, Linux Hyprland, Linux KDE Plasma 6, Linux AppImage y Windows Beta) se documentan en este archivo siguiendo [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/) y [Versionado Semántico](https://semver.org/lang/es/).

---

## [2.1.2] — 2026-11-26

### Añadido / Cambiado
- **Temas renombrados y color de resaltado seleccionable (Escritorio `autoq-gui` + Android)**:
  - Temas renombrados a **`Default`**, **`Default claro`** y **`NothingOS`**.
  - Selector de **color de resaltado** de 8 opciones (`Tema`, `Azul`, `Rojo`, `Verde`, `Violeta`, `Naranja`, `Cian`, `Rosa`) adaptado para modo oscuro y claro, persistente en `ui_settings.json` (`crates/autoq-core/src/config.rs`) y `AppPreferencesStore` en Android.
- **Firma permanente en Android (`assembleRelease`)**:
  - Configuración de `signingConfigs.release` en `android/app/build.gradle.kts` y generación/limpieza de keystore en CI (`.github/workflows/build-releases.yml`) para que todas las versiones futuras se actualicen encima sin desinstalar.

---

## [2.1.1] — 2026-11-26

### Añadido / Cambiado
- **Sistema de temas modulares con paridad total (Escritorio `autoq-gui` + Android)**:
  - **Android / HyperOS** (`PureHyper`, tema principal por defecto): estética limpia con acento azul (`#3482FF` / `#2563EB`), cuadrícula continua en el gráfico EQ y tarjetas sobrias.
  - **HyperOS Claro** (`PureLight`): variante clara modular con alto contraste.
  - **Nothing OS** (`NothingOs`): conservado como tema opcional seleccionable (fondo mate `#0B0C0E`, cabeceras y cuadrícula dot-matrix, acento `#D71921`).
  - Selector de tema persistente en la pestaña *Avanzado* (`crates/autoq-core/src/config.rs`, `autoq-gui` y `AppPreferencesStore` / `AdvancedScreen` en Android).
- **Tipografía Consolas / Inconsolata**:
  - Integración de fuente monoespaciada Consolas (`consolas_regular.ttf`, `consolas_bold.ttf` en Android; `Inconsolata-Regular.ttf` en `autoq-gui`) para valores numéricos, frecuencias, ganancia/Q y diagnósticos.

---

## [2.1.0] — 2026-11-26

### Añadido
- **Daemon residente opcional (`crates/autoq-daemon`)**:
  - Escucha eventos en tiempo real (`udevadm monitor` + `pactl subscribe` con *debounce* en Linux, y huella `poll_fingerprint` en Windows) para aplicar automáticamente el perfil EQ guardado en `~/.config/autoq/daemon_profiles.json` al conectar audífonos o DACs.
  - Subcomandos `--once`, `--dry-run`, `--status`, `--install-service` y `--uninstall-service`, además de unidad [`packaging/systemd/autoq-daemon.service`](./packaging/systemd/autoq-daemon.service) e integración directa en la pestaña *Avanzado* de `autoq-gui`.
- **4 nuevas familias DSP nativas en Rust (8 protocolos soportados en total)**:
  - **JDS Labs Element IV** (`crates/autoq-core/src/jds_serial.rs`, USB-CDC Serial JSON de 12 bandas: `2 LowShelf + 8 Peaking + 2 HighShelf`).
  - **Topping DX1 II / E50 II** (`crates/autoq-core/src/hid.rs`, tramas HID de 16 bytes, 11 bandas L+R y lectura multiframe `0x1106`).
  - **Fosi Audio DS3** (`crates/autoq-core/src/hid.rs`, HID Feature/Output Reports `0x77` de 8 bandas `float32 LE`).
  - **Qudelix-5K** (`crates/autoq-core/src/hid.rs`, comandos HID `SetEqBandParam 0x070F` y parser de bitstream V3 de 10 bandas).
- Regla `udev` USB-CDC `/dev/ttyACM*` para `152a:88fa` (JDS Labs Element IV) añadida a [`packaging/udev/99-autoq-usb-dsp.rules`](./packaging/udev/99-autoq-usb-dsp.rules).
- **Paridad total en Android (`android/`, `v2.1.0`)**:
  - Portados los 6 protocolos faltantes a Kotlin + USB Host (`WalkPlay`, `FiiO 5/10B`, `Qudelix-5K`, `Topping DX1 II / E50 II`, `Fosi Audio DS3` y `JDS Labs Element IV` vía USB-CDC Bulk) completando las **8 familias DSP** en todas las plataformas.
  - Añadido apagado completo del ecualizador y detención total del servicio/app (`GlobalEqService.ACTION_POWER_OFF` + botón **Apagar EQ y detener app/servicio**, liberando sesiones de `DynamicsProcessing` y removiendo la notificación foreground al apagarse).

---

## [2.0.0] — 2026-11-25

### Añadido
- **GUI de escritorio 100 % nativa en Rust (`crates/autoq-gui` con `eframe`/`egui`)**:
  - Estética Nothing OS completa (cabecera de matriz de puntos 5×7, gráfico logarítmico PEQ/FR interactivo con nodos arrastrables y rueda `Q`, modo widget compacto `--widget` ↔ app de 3 pestañas `--full`, IPC por socket Unix `--toggle`).
  - Llamadas directas en memoria a `autoq-core` (cero Python, cero PySide6, cero PyInstaller).
  - Botones de 1 clic integrados en la pestaña *Avanzado* para Widget Hyprland, Plasmoid KDE Plasma 6 y reglas USB `udev`.
- **Motor USB-C HID Multiprotocolo en `crates/autoq-core/src/hid.rs`**:
  - **KTMicro extendido (`0x31B2`)**: Moondrop, Kiwi Ears Allegro Mini/PRO, JCALLY, Tanchjim.
  - **Moondrop 8 bandas**: FreeDSP Pro/Mini, Rays, DAWN PRO 2, Echo A, MOONRIVER 3, DHA15.
  - **WalkPlay (19 VIDs, 5–10 bandas)**: Tanchjim Bunny/Stargate II/Space Pro, CrinEar Protocol Max, NiceHCK, LETSHUOER, ddHifi, Truthear.
  - **FiiO / JadeAudio / SNOWSKY (5–10 bandas)**: JA11, JIEZI, KA15, KA17, BTR13, BTR17, QX13, Q7, BT11, Melody, TINY.
  - Comando de diagnóstico `autoq-cli hid-scan [--all] [--json]` y reglas `udev` multiprotocolo ampliadas.
- **GitHub Actions activado** en `.github/workflows/build-releases.yml` (tests Rust en cada push/PR + compilación automática 100 % Rust de AppImage, `.deb`, Windows `.zip`, Android `.apk` y `SHA256SUMS.txt`).
- **Capturas reales de la estética Nothing OS** añadidas en `docs/screenshots/` y mostradas en `README.md`.

### Cambiado
- `AutoQ-x86_64.AppImage`, `autoq_2.0.0_amd64.deb`, `packaging/aur/PKGBUILD` y `windows-beta/build_windows.ps1` pasan a ser **100 % Rust nativo** (AppImage reducido de `128 MB` a `< 10 MB`).

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
