# Roadmap de Trivium (antes AutoQ)

Este documento registra los hitos completados del proyecto y la hoja de ruta técnica para **Trivium**, elaborada a partir de la auditoría directa del código base (*Rust workspace* + *Android Kotlin/Compose*) y las recomendaciones de calidad, seguridad y madurez multiplataforma.

---

## Hitos Completados (`v1.1.0` → `v2.1.2`)

### Fase 1 — `v1.1.0`: Motor Nativo Unificado en Rust (`autoq-core` + `autoq-cli`) — ✅ Completada
- [x] Workspace Cargo (`Cargo.toml`) con `crates/autoq-core` y `crates/autoq-cli`.
- [x] Motor matemático RBJ/PEQ (`biquad.rs`) en `< 0.5 ms` eliminando `numpy`.
- [x] Motor AutoEQ concurrente de 8 fuentes (+14,500 curvas) con caché SHA-1 retrocompatible y atribución visible (`autoq-cli eq`).
- [x] Backends nativos de PipeWire 10B, Bluetooth Classic SPP RFCOMM y Equalizer APO, más telemetría sub-milisegundo (`poll` en `~1 ms` y `waybar-status`).
- [x] Migración automática de `~/.config/chu2-dsp-widget` → `~/.config/autoq`.

### Fase 2 — `v2.0.0`: GUI 100 % Nativa en Rust, DSP Multiprotocolo y CI/CD — ✅ Completada
- [x] **Expansión USB-C HID inicial (`crates/autoq-core/src/hid.rs`)**: KTMicro 5B, Moondrop 8B, WalkPlay (19 VIDs, 5–10B) y FiiO / JadeAudio / SNOWSKY (5–10B), más `autoq-cli hid-scan --all`.
- [x] **GUI de Escritorio 100 % Nativa en Rust (`crates/autoq-gui` con `eframe`/`egui`)**: gráfico interactivo PEQ/FR arrastrable con rueda `Q`, modo widget compacto ↔ app de 3 pestañas, IPC Hyprland e instaladores 1-clic en *Avanzado*.
- [x] Eliminación total de Python / PySide6 / PyInstaller en AppImage, `.deb`, `AUR` y Windows.
- [x] **CI/CD oficial** en [`.github/workflows/build-releases.yml`](./.github/workflows/build-releases.yml) con pruebas Rust y publicación automatizada multi-plataforma (`AppImage`, `.deb`, binarios sueltos, Windows `.zip`, Android `.apk` y `SHA256SUMS.txt`).

### Fase 3 — `v2.1.0` – `v2.1.2`: Daemon Auto-Perfil, 8 Familias DSP, Temas Modulares y Firma Android — ✅ Completada
- [x] **Daemon residente opcional (`crates/autoq-daemon`)**: escucha `udevadm monitor` + `pactl subscribe` en Linux (y sondeo en Windows) para aplicar automáticamente perfiles guardados en `daemon_profiles.json` + unidad `systemd --user`.
- [x] **8 familias DSP completas (Escritorio + paridad Android)**: añadidas **JDS Labs Element IV** (12B USB-CDC Serial JSON), **Topping DX1 II / E50 II** (11B L+R HID + CRC-16), **Fosi Audio DS3** (8B HID `float32 LE`) y **Qudelix-5K** (10B HID bitstream V3).
- [x] **Limpieza de legado v1** y paridad funcional en Android (botón y notificación para apagar el EQ y detener por completo `GlobalEqService` y la app).
- [x] **Sistema de temas modulares + color de resaltado + Consolas (`v2.1.2`)**: temas **`Default`**, **`Default claro`** y **`NothingOS`** (opcional), selector de 8 colores de resaltado y tipografía Consolas / Inconsolata en `autoq-gui` y Android.
- [x] **Firma permanente de APK `release` en CI (`v2.1.2`)**: `signingConfigs.release` en `android/app/build.gradle.kts` con secretos cifrados en GitHub Actions para permitir actualizaciones directas encima en Android sin desinstalar.

---

## Próximas Fases

### Fase 4 — `v2.2.0`: Seguridad, Confianza en Windows, Higiene de Workspace y Documentación — ✅ Integrada (salvo trámites externos)

> Objetivo: cerrar los hallazgos técnicos, de confianza de binarios y de seguridad detectados en la revisión del repositorio sin añadir complejidad innecesaria.

- [x] **Experiencia limpia e instalador único en Windows (`AutoQ-Setup-x64.exe`)**:
  - Eliminado el lanzamiento de `powershell.exe` al escanear JDS Labs Element IV (reemplazado por consulta directa al Registro de Windows vía `winreg`, cero ventanas de terminal emergentes), protegido `platform_linux::run_cmd` fuera de Linux, embebido ícono/metadatos PE nativos (`winresource`) e integrado instalador único con Inno Setup 6 y *hook* `signtool` en CI.
- [ ] **Firma Authenticode gratuita para Windows (SignPath Foundation) y envío de PR a WinGet**:
  - Registrar el proyecto en **SignPath Foundation** y enviar el manifiesto preparado ([`packaging/winget/Speed17x.AutoQ.yaml`](./packaging/winget/Speed17x.AutoQ.yaml), guía en [`packaging/winget/README.md`](./packaging/winget/README.md)) a **`microsoft/winget-pkgs`** para habilitar `winget install Speed17x.AutoQ`.

- [x] **Endurecimiento de permisos `udev` (`MODE="0660", TAG+="uaccess"`)**:
  - Reemplazado `MODE="0666"` por `MODE="0660", TAG+="uaccess"` en [`packaging/udev/99-autoq-usb-dsp.rules`](./packaging/udev/99-autoq-usb-dsp.rules) y unificado mediante `include_str!` en el instalador 1-clic de `autoq-gui`, restringiendo `/dev/hidraw*` y `/dev/ttyACM*` al usuario físico activo mediante `systemd-logind`.
- [x] **Licencia raíz formal, metadatos de GitHub y limpieza de sub-READMEs**:
  - Añadido [`LICENSE`](./LICENSE) (`GPL-2.0-or-later`) en la raíz, configurados descripción corta y *topics* en `Speed17x/AutoQ` y actualizados [`linux-hyprland/README.md`](./linux-hyprland/README.md), [`android/README.md`](./android/README.md), [`packaging/README.md`](./packaging/README.md), [`linux-appimage/README.md`](./linux-appimage/README.md) y [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md) al flujo 100 % Rust (sin `python-pyside6`, `pyinstaller` ni referencias fijas a `1.1.0`).
- [x] **Seguridad de suministro en instalador y CI/CD**:
  - Actualizado [`install-linux.sh`](./install-linux.sh) para verificar todos los binarios contra `SHA256SUMS.txt` antes de instalar (soportando `x86_64` y `aarch64`) e instalando `autoq-daemon`.
  - Reducido `permissions` global en [`.github/workflows/build-releases.yml`](./.github/workflows/build-releases.yml) a `contents: read` (dejando `contents: write` únicamente en `publish-release`).
- [x] **Higiene del *workspace* Cargo y encapsulación de `unsafe`**:
  - Centralizadas las dependencias compartidas bajo `[workspace.package]` y `[workspace.dependencies]` en [`Cargo.toml`](./Cargo.toml).
  - Aisladas todas las llamadas POSIX `libc` tras el *wrapper* RAII seguro [`crates/autoq-core/src/posix_fd.rs`](./crates/autoq-core/src/posix_fd.rs) (`OwnedFd`), endurecidos los *clamps* contra `NaN`/`±∞` y ampliadas las pruebas unitarias con paquetes HID/JDS truncados y malformados.

---

### Fase 5 — `v2.2.0`: Coherencia Visual en Widgets, Herramientas Audiófilas y Distribución — ✅ Integrada (salvo trámites externos)

- [x] **Widgets sincronizados con los temas modulares**:
  - Adaptado `NothingEqWidgetProvider` + `NothingDotMatrixRenderer` en Android (fondos, bordes, color de acento dinámico y título limpio sin matriz de puntos en `Default` / `Default claro`) y el Plasmoid QML de KDE Plasma 6 (`linux-kde/plasmoid/contents/ui/main.qml`) leyendo `ui_settings` desde `autoq-cli status`.
- [x] **Funciones de ecualización avanzada (Escritorio + paridad Android)**:
  - Control manual explícito de **Preamp** (dB) con modo `AUTO` y medidor visual de margen anti-*clipping* aplicado tanto a PipeWire (`mixer`), Equalizer APO y Android `GlobalEqService`.
  - Comparación instantánea **A / B** entre dos curvas y preamps (`Slot A` / `Slot B` + copia `A→B` / `B→A`) en la pestaña **EQ** sin tocar la memoria flash.
  - Exportación multi-formato adicional hacia **EasyEffects** (JSON), **CamillaDSP** (YAML) y **Wavelet** (`GraphicEQ:` de 127 bandas normalizado a ≤ 0 dB) además de `ParametricEQ.txt` (en GUI, Android y CLI `--format` / `export-format`).
- [x] **Evaluación de núcleo compartido `autoq-core` en Android (`JNI` / `UniFFI`)**:
  - Documentado el análisis técnico completo y veredicto por etapas en [`docs/ANDROID_RUST_CORE_EVAL.md`](./docs/ANDROID_RUST_CORE_EVAL.md).
- [x] **Compilaciones Linux ARM64 (`aarch64`) y perfil Obtainium**:
  - Añadido job `build-linux-arm64` (`ubuntu-24.04-arm`) en GitHub Actions generando `autoq-{gui,cli,daemon}-linux-aarch64` con soporte nativo en `install-linux.sh`.
  - Publicada configuración lista para importar en **Obtainium** en [`packaging/android/obtainium-autoq.json`](./packaging/android/obtainium-autoq.json).
- [ ] **Canales externos pendientes de publicación (AUR y Flatpak)**:
  - Publicar `autoq` / `autoq-bin` en **AUR** (`aur.archlinux.org`) cuando se habilite el registro de cuenta y explorar empaquetado **Flatpak**.

---

### Fase 6 — `v3.0.0`: Rebranding a **Trivium**, Ganancia/Atenuación de Salida, Conexant 9B, BLE GATT y macOS ARM64 — ✅ Integrada

- [x] **Identidad propia (`AutoQ` → `Trivium`)**:
  - Renombrado completo de repositorio (`Speed17x/Trivium`), crates (`trivium-core`, `trivium-cli`, `trivium-gui`, `trivium-daemon`), binarios (`trivium`), paquete Android independiente (`com.speed17x.trivium`), Plasmoid (`org.speed17x.trivium`), reglas `udev`, Hyprland, Windows, Obtainium (`obtainium-trivium.json`) y WinGet (`Speed17x.Trivium.yaml`), con migración transparente desde `~/.config/autoq`.
- [x] **Ganancia / Atenuación de Salida (`-30.0` a `+12.0 dB`, Android + Escritorio)**:
  - Control dedicado de salida separado del Preamp anti-*clipping*: aplicado mediante `DynamicsProcessing.Limiter.postGain` en Android `GlobalEqService` (ideal para atenuar el volumen en IEMs sensibles), nodo `eq_output_gain` en PipeWire y etapa final `Preamp:` en Equalizer APO.
- [x] **Reporte comunitario y matriz de hardware ([`docs/HARDWARE_MATRIX.md`](./docs/HARDWARE_MATRIX.md))**:
  - Comando `trivium-cli hid-scan --dump-descriptors` de solo lectura (con `UsagePage`, `Usage` y descriptor HID hexadecimal completo en Linux).
- [x] **9.ª familia USB-C DSP (Conexant Freeman 9B)**:
  - Soporte completo para **Moondrop FreeDSP** (`0x35D8:0x1496`) y **Moondrop ECHO-B** (`0x35D8:0x149B`) en Rust y Android con cálculo biquad cuantizado $2^{30}$ para 44.1 / 48 / 96 / 192 kHz.
- [x] **Transporte inalámbrico BLE GATT DSP**:
  - Codecs verificados para **FiiO BLE GATT** (`FIIO EH11` / `EH13`, 10B ±20 dB) y **Airoha BLE GATT** (`Audeze Maxwell`, 10B 193B) en [`crates/trivium-core/src/ble_dsp.rs`](./crates/trivium-core/src/ble_dsp.rs) y transporte nativo Android [`BleDspTransport.kt`](./android/app/src/main/kotlin/com/speed17x/trivium/hardware/BleDspTransport.kt).
- [x] **Soporte experimental macOS (`aarch64-apple-darwin`)**:
  - Backend `hidapi` (`macos-shared-device`) separado por target en Cargo y job `build-macos-arm64` en GitHub Actions generando `Trivium-macOS-arm64.zip`.
