# Roadmap de AutoQ (Estrategia *Rust-First*)

Este documento define la hoja de ruta técnica del monorepo **AutoQ**, priorizando la migración progresiva a **Rust** allí donde ofrece ventajas reales y medibles en latencia, consumo de memoria, seguridad binaria USB HID y tamaño de distribución.

---

## Principios de arquitectura para la migración a Rust

| Capa | Estrategia | Justificación técnica |
| :--- | :--- | :--- |
| **Motor DSP, HID, AutoEQ y CLI (`crates/autoq-core`, `crates/autoq-cli`)** | **🟢 Migrar a Rust (Prioridad #1)** | Arranque `< 2 ms` (~2 MB RSS) para Waybar y KDE Plasma; optimizador biquad ×80 más rápido sin `numpy`; un único driver `hidapi` seguro (5 y 8 bandas) para Linux y Windows; jubila `moondrop_poll.c` y elimina la triplicación del backend Python. |
| **Empaquetado y CI/CD (`AppImage`, `Windows .exe`, `Android .apk`)** | **🟢 Integración híbrida inmediata** | Empaquetar `autoq-cli` compilado en modo `release` dentro del AppImage y el portable de Windows permite eliminar `numpy` y `requests` de PyInstaller de inmediato. |
| **GUI de Escritorio (`PySide6` → `Slint` / `Iced` / `Tauri v2`)** | **🟡 Fase posterior (`v2.0.0`)** | Mantener temporalmente la interfaz Nothing OS probada en Qt6/Wayland mientras madura `autoq-core`, y luego reemplazar PySide6 para bajar los binarios de escritorio de ~128 MB a `< 15 MB`. |
| **App Android (`android/`)** | **⚪ Mantener en Kotlin + Jetpack Compose** | Kotlin ya compila nativamente en ART y accede sin fricción a `UsbManager`, `BluetoothSocket` y `GlobalEqService` sin sobrecarga de JNI/NDK. |

---

## Fase 1 — `v1.1.0`: Motor Nativo Unificado en Rust (`autoq-core` + `autoq-cli`) *(En curso)*

- [x] **Workspace Cargo en la raíz (`Cargo.toml`)** con dos crates:
  - `crates/autoq-core`: librería multiplataforma sin dependencias pesadas.
  - `crates/autoq-cli`: binario único con contrato JSON 100 % compatible con `moondrop_backend.py` y `moondrop_backend_win.py`.
- [x] **Matemáticas DSP y optimizador PEQ en Rust puro (`biquad.rs`)**:
  - Filtros RBJ Audio EQ Cookbook (`peaking`, `low_shelf`, `high_shelf`), evaluación logarítmica de 128 puntos y ajuste iterativo `fit_peq_bands` en `< 0.5 ms` (elimina `numpy`).
  - Cálculo automático de *preamp* anti-clipping.
- [x] **Driver USB-C HID multiplataforma en Rust (`hid.rs`)**:
  - Soporte unificado en **Linux y Windows** tanto para **KTMicro 5 bandas** (`VID 0x31B2`: Chu 2 DSP, CDSP, Quark 2, Jiu, May) como para **Moondrop 8 bandas** (*FreeDSP Pro/Mini*, *Rays*, *AG Rays*, *Marigold*, *DAWN PRO 2*, *Echo A*, *MOONRIVER 3*, *DHA15*).
  - Verificación de lectura/escritura antes de hacer `COMMIT` a memoria flash.
- [x] **Motor AutoEQ concurrente de 8 fuentes (`autoeq.rs`)**:
  - Descarga paralela y caché local (`~/.cache/autoq` con lectura retrocompatible de `~/.cache/chu2-dsp-widget`) para +14,500 curvas (*AutoEq GitHub* + 7 repositorios *Squiglink*).
  - Modo hacia curva objetivo (8 targets curados) y modo «Imitar cualquier audífono», más importación/exportación `ParametricEQ.txt`.
- [x] **Backends de audio del sistema y telemetría instantánea**:
  - **Linux (`platform_linux.rs`)**: generación y control de `libpipewire-module-filter-chain` (10 bandas), detección de salidas `pactl`, Bluetooth Classic SPP RFCOMM nativo y subcomandos ultrarrápidos `autoq-cli poll` (reemplaza `moondrop_poll.c`) y `autoq-cli waybar-status`.
  - **Windows (`platform_windows.rs`)**: integración nativa con **Equalizer APO** (`autoq_eq.txt` + `Include` automático).
  - **Migración limpia de configuración (`config.rs`)**: transición a `~/.config/autoq` conservando automáticamente los presets existentes en `~/.config/chu2-dsp-widget`.
- [x] **Integración *drop-in* en las interfaces actuales**:
  - Conectar `linux-hyprland`, `linux-kde` (Plasmoid QML + scripts), `linux-appimage` y `windows-beta` a `autoq-cli`, retirando `numpy` y `requests` del empaquetado PyInstaller.

---

## Fase 2 — `v1.1.1`: CI/CD Completo, Pulido de AppImage y Experiencia 1-Clic

- [ ] **Activar GitHub Actions (`.github/workflows/build-releases.yml`)**:
  - Mover y actualizar la plantilla de `docs/github-actions-build-releases.yml` para ejecutar `cargo test --workspace`, compilar `autoq-cli --release` en Linux y Windows, empaquetar `AutoQ-x86_64.AppImage`, `AutoQ-Windows-Beta-x64.zip` y `AutoQ-Android.apk`, y publicar `SHA256SUMS.txt` en cada tag `v*`.
  - Refrescar el token local con `gh auth refresh -h github.com -s workflow` para habilitar el push de workflows.
- [x] **Pulir `AutoQ-x86_64.AppImage` y experiencia de instalación en Linux**:
  - Icono PNG `256x256` (`.DirIcon`) generado automáticamente en `build_appimage.sh` + exclusión de `numpy`, `requests` y módulos ociosos de `PySide6`.
  - Botones de **1 clic en la pestaña *Avanzado*** para instalar los widgets incluidos (Hyprland + Waybar, Plasmoid KDE Plasma 6 y reglas `udev` vía `pkexec`).
  - Comando interactivo de terminal `autoq-cli eq` con atribución visible de la fuente (`AutoEq` / `Squiglink`).
  - Empaquetado nativo para gestores de paquetes Linux (`packaging/aur/PKGBUILD` para `paru`/`pacman`, `packaging/deb/build_deb.sh` para `apt` `.deb`, e instalador en 1 línea `install-linux.sh`).
- [ ] **Capturas reales en `README.md`**:
  - Generar capturas reales de la estética Nothing OS (`--screenshot-demo` en escritorio + capturas de Android) e integrarlas en el `README.md` principal.

---

## Fase 3 — `v1.2.0`: Consolidación de UI y Validación Hardware en Windows

- [ ] **Unificar la capa visual PySide6**: extraer las 2,431 líneas compartidas entre `linux-hyprland/autoq_hyprland.py` y `windows-beta/autoq_ui.py` a un único módulo de interfaz común que delegue el 100 % de la lógica en `autoq-cli`.
- [ ] **Validación en hardware Windows 10/11 real**: probar lectura/escritura HID (5 y 8 bandas con `autoq-cli.exe`) y Equalizer APO en equipos Windows físicos para retirar la etiqueta `BETA — SIN TESTEAR`.
- [ ] **Daemon opcional en Rust (`autoq-daemon`)**: modo residente ultraligero (`--daemon`) que escuche eventos `udev` / PipeWire en tiempo real y aplique automáticamente el perfil correspondiente al conectar cada audífono USB-C o Bluetooth.

---

## Fase 4 — `v2.0.0`: GUI de Escritorio 100 % Nativa en Rust (Cero Python)

- [ ] **Portar la interfaz Nothing OS de escritorio a Rust nativo** (evaluando `Slint` o `Iced` con soporte Wayland `wlr-layer-shell` para Hyprland y ejecutable nativo en Windows):
  - Eliminar por completo `Python` y `PySide6` de las distribuciones de escritorio.
  - Reducir `AutoQ-x86_64.AppImage` y `AutoQ-Windows-x64.exe` de >100 MB a **< 12 MB** con arranque instantáneo (`< 15 ms`).
  - Mantener el Plasmoid nativo QML en KDE Plasma 6 alimentado directamente por `autoq-cli`.
