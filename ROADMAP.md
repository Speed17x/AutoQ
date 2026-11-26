# Roadmap de AutoQ (Estrategia *Rust-First*)

Este documento registra el progreso y la evolución técnica del monorepo **AutoQ**, priorizando **Rust nativo** en escritorio (Linux y Windows) y **Kotlin + Jetpack Compose** en Android.

---

## Fase 1 — `v1.1.0`: Motor Nativo Unificado en Rust (`autoq-core` + `autoq-cli`) — ✅ Completada

- [x] Workspace Cargo (`Cargo.toml`) con `crates/autoq-core` y `crates/autoq-cli`.
- [x] Motor matemático RBJ/PEQ (`biquad.rs`) en `< 0.5 ms` eliminando `numpy`.
- [x] Motor AutoEQ concurrente de 8 fuentes (+14,500 curvas) con caché SHA-1 retrocompatible y atribución visible (`autoq-cli eq`).
- [x] Backends nativos de PipeWire 10B, Bluetooth Classic SPP RFCOMM y Equalizer APO, más telemetría sub-milisegundo (`poll` en `1 ms` y `waybar-status`).
- [x] Migración automática de `~/.config/chu2-dsp-widget` → `~/.config/autoq`.

---

## Fase 2 — `v2.0.0`: GUI 100 % Nativa en Rust, DSP Multiprotocolo y CI/CD Completo — ✅ Completada

- [x] **Expansión USB-C HID Multiprotocolo (`crates/autoq-core/src/hid.rs`)**:
  - **KTMicro 5 bandas (`0x31B2`)**: Moondrop, Kiwi Ears Allegro Mini/PRO, JCALLY, Tanchjim KTMicro.
  - **Moondrop 8 bandas**: FreeDSP Pro/Mini, Rays, AG Rays, Marigold, DAWN PRO 2, Echo A, MOONRIVER 3, DHA15.
  - **WalkPlay (19 VIDs, 5–10 bandas)**: Tanchjim Bunny/Stargate II/Space Pro, CrinEar Protocol Max, NiceHCK, LETSHUOER, ddHifi, Truthear.
  - **FiiO / JadeAudio / SNOWSKY (5–10 bandas)**: JA11, JIEZI, KA15, KA17, BTR13, BTR17, QX13, Q7, BT11, Melody, TINY.
  - Comando de diagnóstico `autoq-cli hid-scan --all` y reglas `udev` ampliadas ([`packaging/udev/99-autoq-usb-dsp.rules`](./packaging/udev/99-autoq-usb-dsp.rules)).
- [x] **GUI de Escritorio 100 % Nativa en Rust (`crates/autoq-gui` con `eframe`/`egui`)**:
  - Estética Nothing OS (matriz de puntos 5×7, gráfico interactivo PEQ/FR arrastrable con rueda `Q`, modo widget compacto ↔ app completa de 3 pestañas, IPC Hyprland).
  - Cero Python / PySide6 / PyInstaller en AppImage, `.deb`, `AUR` y Windows (`AutoQ.exe`).
  - Botones 1-clic integrados en *Avanzado* para Widget Hyprland, Plasmoid KDE 6 y reglas `udev`.
- [x] **CI/CD oficial activado en [`.github/workflows/build-releases.yml`](./.github/workflows/build-releases.yml)**:
  - Ejecuta `cargo test --workspace` en cada push/PR, y compila automáticamente AppImage 100 % Rust, `.deb`, binarios sueltos, Windows `.zip`, Android `.apk` y `SHA256SUMS.txt` en cada tag `v*`.
- [x] **Capturas reales Nothing OS** generadas en `docs/screenshots/` e integradas en [`README.md`](./README.md).

---

## Fase 3 — `v2.1.0`: Daemon Auto-Perfil y Protocolos DSP Avanzados — 🚀 Código Completado

- [x] **Daemon residente opcional (`crates/autoq-daemon`)**: escucha eventos `udevadm monitor` + `pactl subscribe` en Linux (y sondeo ligero en Windows), aplicando automáticamente reglas por dispositivo (`~/.config/autoq/daemon_profiles.json`) o perfiles PipeWire guardados al conectar audífonos; incluye unidad `systemd --user` y gestión en 1 clic desde *Avanzado*.
- [x] **Protocolos DSP adicionales (8 familias en total)**:
  - **JDS Labs Element IV (`crates/autoq-core/src/jds_serial.rs`)**: USB-CDC Serial JSON (`115200 8N1`, `Describe` / `Update` 12 bandas `2 LSQ + 8 PK + 2 HSQ`).
  - **Topping DX1 II / E50 II (`crates/autoq-core/src/hid.rs`)**: HID 11 bandas L+R en tramas de 16 bytes (`0x22 0x33 ... 0x66 0x77 0x00`, broadcast `upload 0x1106` y CRC-16/MODBUS).
  - **Fosi Audio DS3 (`crates/autoq-core/src/hid.rs`)**: HID 8 bandas (`0x77` Feature/Output reports, `float32 LE`).
  - **Qudelix-5K (`crates/autoq-core/src/hid.rs`)**: HID 10 bandas (decodificador bitstream V3 de 88 bytes + `SetEqBandParam 0x070F`).
- [x] **Paridad Android (`v2.1.0`)**: portados los 6 protocolos nuevos a Kotlin/USB Host (8 familias completas) + botón y acción de notificación para apagar el EQ y detener por completo `GlobalEqService` y la app.
- [ ] **Publicación oficial en AUR (`paru -S autoq-bin`)**: en cuanto Arch Linux reabra el registro de cuentas nuevas en `aur.archlinux.org` (llave SSH y `PKGBUILD` + `.SRCINFO` ya preparados).
- [ ] **Validación física en Windows 10/11 y más hardware DSP**: pruebas en hardware real para `AutoQ.exe` (`hidapi` + Equalizer APO) y las familias JDS Labs / Topping / Fosi / Qudelix / FiiO / WalkPlay.
