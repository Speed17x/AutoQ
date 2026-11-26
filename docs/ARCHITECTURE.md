# Arquitectura técnica de AutoQ

AutoQ comparte el mismo modelo conceptual, matemáticas de filtros RBJ y protocolos binarios entre su **workspace 100 % nativo en Rust** para escritorio (`autoq-core`, `autoq-gui`, `autoq-cli`, `autoq-daemon` en Linux y Windows) y su **motor nativo Kotlin** en Android.

```mermaid
flowchart TD
    UI_AND["Android UI<br/>(Jetpack Compose + Widget Temático)"]
    UI_DESK["Desktop GUI<br/>(autoq-gui 100% Rust eframe/egui)"]
    UI_KDE["KDE Plasma 6 / Waybar<br/>(QML Plasmoid + IPC Hyprland)"]
    DAEMON["Daemon Residente<br/>(autoq-daemon · udev + pactl)"]

    CORE_KT["Motor DSP Kotlin<br/>BiquadMath + AutoEqRepository"]
    CORE_RS["Workspace Nativo Rust<br/>crates/autoq-core + autoq-cli"]

    HID["8 Familias USB-C DSP<br/>KTMicro · MD8 · WalkPlay · FiiO · Qudelix · Topping · Fosi · JDS"]
    SPP["Bluetooth Classic SPP<br/>RFCOMM (POP / MON / BASS / PEQ)"]
    PW["PipeWire filter-chain<br/>Preamp mixer + 10B PEQ (Linux)"]
    APO["Equalizer APO<br/>Preamp + 10B PEQ globales (Windows)"]
    GFX["Android GlobalEqService<br/>DynamicsProcessing 32B del sistema"]

    DB["8 Fuentes Abiertas<br/>AutoEq GitHub + 7 Squiglink (+14,500 curvas)"]

    UI_AND --> CORE_KT
    UI_DESK --> CORE_RS
    UI_KDE --> CORE_RS
    DAEMON --> CORE_RS

    CORE_KT --> HID
    CORE_KT --> SPP
    CORE_KT --> GFX
    CORE_KT --> DB

    CORE_RS --> HID
    CORE_RS --> SPP
    CORE_RS --> PW
    CORE_RS --> APO
    CORE_RS --> DB
```

---

## 1. Workspace Nativo en Rust (`crates/*`)
- **`crates/autoq-core`**: optimizador RBJ/PEQ, cálculo de headroom/preamp anti-clipping, cliente concurrente de 8 bases AutoEQ + Squiglink, exportadores (`EqualizerAPO`, `EasyEffects JSON`, `CamillaDSP YAML`, `Wavelet GraphicEQ`), backends `PipeWire` y `Equalizer APO`, acceso serial/USB HID y cápsula POSIX RAII segura (`posix_fd.rs` basada en `OwnedFd`).
- **`crates/autoq-gui`**: interfaz nativa multiplataforma en `eframe`/`egui` con temas modulares (**`Default`**, **`Default claro`**, **`NothingOS`**), 8 colores de resaltado, comparación instantánea **A / B**, gráfica interactiva, instaladores 1-clic y socket Unix IPC (`--toggle`, `--widget`).
- **`crates/autoq-cli`**: binario JSON/CLI ultraligero con subcomandos de telemetría sub-milisegundo (`poll`, `waybar-status`) y exportación multi-formato (`eq --format`, `export-format`).
- **`crates/autoq-daemon`**: demonio residente opcional que escucha eventos `udevadm monitor` + `pactl subscribe` en Linux (y sondeo en Windows vía Registro nativo sin abrir ventanas de consola) para aplicar perfiles guardados automáticamente.

## 2. Motor Matemático DSP (`biquad.rs` / `BiquadMath.kt`)
- Calcula la respuesta en magnitud (dB) sobre rejillas logarítmicas entre **20 Hz y 20,000 Hz** (`standard_grid` y rejilla de 256 puntos para `peak_boost_db` / `headroom_margin_db`).
- Implementa filtros **Audio EQ Cookbook (RBJ)**: `Peaking` (`PK`), `LowShelf` (`LS` / `LSC`) y `HighShelf` (`HS` / `HSC`).
- Protege contra valores no finitos (`NaN` / `±∞`) antes de cuantizar a la resolución de cada protocolo hardware.
- Ajusta curvas objetivo mediante búsqueda greedy de residuales + descenso coordinado conjunto en **`< 0.5 ms` en Rust**.

## 3. Motor AutoEQ Multi-fuente y Exportadores (`autoeq.rs` / `AutoEqRepository.kt`)
- Descarga en paralelo los índices de **8 repositorios abiertos**:
  1. `jaakkopasanen/AutoEq` (GitHub)
  2. `Super* Review` (`squig.link/data/`)
  3. `ReganCipher` (`regancipher.squig.link/data/`)
  4. `Aftersound` (`aftersound.squig.link/data/`)
  5. `HBB` (`hbb.squig.link/data/`)
  6. `Paul Wasabii` (`pw.squig.link/data/`)
  7. `Jaytiss` (`jaytiss.squig.link/data/`)
  8. `GadgetryTech` (`gadgetrytech.squig.link/headsets/data/`)
- Normaliza nombres y puntuación (`Zero:3` → `zero 3`) con caché local (`~/.cache/autoq`).
- Exporta directamente a **EqualizerAPO** (`ParametricEQ.txt`), **EasyEffects** (`v7 IIR JSON`), **CamillaDSP** (`v2 YAML`) y **Wavelet** (`GraphicEQ:` de 127 bandas normalizado a ≤ 0 dB).

## 4. Capas de salida
1. **Hardware USB-C HID / Serial (`hid.rs`, `jds_serial.rs`)**: `/dev/hidraw*` y `/dev/ttyACM*` vía descriptores RAII `OwnedFd` en Linux; `hidapi` + `winreg` (cero subprocesses) en Windows; `UsbManager` en Android. Cubre **KTMicro**, **Moondrop 8B**, **WalkPlay**, **FiiO**, **Qudelix-5K**, **Topping**, **Fosi DS3** y **JDS Labs Element IV**.
2. **PipeWire `filter-chain` (`platform_linux.rs`)**: nodo previo `mixer` de preamp anti-clipping + cadena de 10 bandas `bq_*` en Linux.
3. **Equalizer APO (`platform_windows.rs`)**: generación de `autoq_eq.txt` con preamp automático o manual en Windows 10/11.
4. **Android `GlobalEqService`**: ecualización paramétrica global a nivel de sistema (`DynamicsProcessing` 32B) con compensación de preamp por app o sesión 0.
5. **Bluetooth Classic SPP**: sockets RFCOMM nativos en Linux y `BluetoothSocket` en Android.
