# Matriz de Hardware y Protocolos — **Trivium (`v3.0.0`)**

Estado de compatibilidad entre familias USB-C HID/CDC, transporte inalámbrico BLE GATT, Bluetooth clásico SPP y motores de EQ del sistema.

> **Leyenda de estado:** ✅ **Hardware real probado** · 🧪 **Implementado por protocolo + tests unitarios (abierto a pruebas comunitarias)**
>
> **Cómo ayudar a ampliar esta lista:** conecta tu DAC, cable DSP o audífono y ejecuta:
> ```bash
> trivium-cli hid-scan --dump-descriptors --json
> ```
> Abre un issue en [Speed17x/Trivium](https://github.com/Speed17x/Trivium/issues) pegando el JSON resultante (incluye `vid:pid`, `usage_page` y descriptor hex sin abrir ni modificar el dispositivo).

---

## 1. Hardware USB-C HID / Serial (9 Familias)

| Familia / Chipset | Modelos principales | Bandas | Filtros | Rango | Flash | Estado de verificación |
| :--- | :--- | :---: | :--- | :---: | :---: | :--- |
| **KTMicro** (`VID 0x31B2`) | Moondrop Chu 2 DSP (`0x0113`), May, Quark 2, Jiu, Kiwi Ears Allegro Mini/PRO, JCALLY JM12, Tanchjim Zero/One/Tanya DSP | 5 | PK / LS / HS | ±12 dB | Sí (`0x53`, reconecta) | ✅ **Verificado en hardware real (Chu 2 DSP)** + pruebas unitarias |
| **Moondrop 8-Band** | FreeDSP Pro, FreeDSP Mini, Rays, AG Rays, Marigold, DAWN PRO 2, Echo A, MOONRIVER 3, DHA15 | 8 | PK / LS / HS (según modelo) | ±6 / ±12 dB | Sí (`0x01`) | 🧪 Implementado por protocolo (`devicePEQ`) + pruebas unitarias · **Abierto a pruebas comunitarias** |
| **Conexant Freeman** (`VID 0x35D8`) | Moondrop FreeDSP (`0x1496`), Moondrop ECHO-B (`0x149B`) | 9 (fijas) | PK / LS / HS | ±12 dB | Directo (4 tasas SR) | 🧪 Implementado por protocolo (`devicePEQ`) + pruebas unitarias · Solo escritura |
| **WalkPlay** (`19 VIDs`) | Tanchjim Bunny / Stargate II / Space Pro, CrinEar Protocol Max (`10B`), NiceHCK, LETSHUOER, ddHifi, Truthear | 5–10 | PK / LS / HS | ±10 / ±12 dB | Sí (`0x01 0x01`) | 🧪 Implementado por protocolo (`devicePEQ`) + pruebas unitarias · **Abierto a pruebas comunitarias** |
| **FiiO / JadeAudio / SNOWSKY** (`0x2972`, `0x0A12`) | JA11 (`5B`), JIEZI (`5B`), KA15, KA17, BTR13, BTR17, QX13, Q7, BT11, SNOWSKY Melody / TINY | 5–10 | PK / LS / HS | ±12 dB | Sí (`0x19` / `0x21`) | 🧪 Implementado por protocolo (`devicePEQ`) + pruebas unitarias · **Abierto a pruebas comunitarias** |
| **Qudelix-5K / 5K+** (`VID 0x0A12`) | Qudelix-5K / 5K+ | 10 | PK / LS / HS | ±12 dB | Sí (`0x0708`) | 🧪 Implementado por protocolo (Bitstream V3 + `SetEqBandParam`) + unit tests · **Abierto a pruebas comunitarias** |
| **Topping PEQ** (`VID 0x152A`) | Topping DX1 II, E50 II, D50 III, DX5 II | 11 | PK / LS / HS | ±12 dB | Vista previa (`0x111B`) | 🧪 Implementado por protocolo (CRC-16/MODBUS) + unit tests · **Abierto a pruebas comunitarias** |
| **Fosi Audio** (`VID 0x152A`) | Fosi Audio DS3, ZH3, K7 | 8 | PK / LS / HS | ±12 dB | Sí (`0x92`) | 🧪 Implementado por protocolo (`0x77`) + unit tests · **Abierto a pruebas comunitarias** |
| **JDS Labs USB-CDC** (`0x152A:0x88FA`) | JDS Labs Element IV | 12 | LS + 10 PK + HS | ±12 dB | Inmediato (JSON) | 🧪 Implementado por protocolo (Serial JSON `Describe`/`Update`) + unit tests · **Abierto a pruebas comunitarias** |

---

## 2. Transporte Inalámbrico BLE GATT & Bluetooth Clásico SPP

| Protocolo | Modelos compatibles | Plataformas | Capacidades | Estado |
| :--- | :--- | :--- | :--- | :--- |
| **FiiO BLE GATT** (`00001100-...`) | FiiO EH11, FiiO EH13 | **Android** (`BleDspTransport` nativo) · **Desktop Core** (codecs + `trivium-cli ble-protocols`) | 10 bandas · ±20 dB · Lectura y escritura por GATT RX/TX | 🧪 Implementado por protocolo (`devicePEQ`) + pruebas unitarias · **Abierto a pruebas comunitarias** |
| **Airoha BLE GATT** (`5052494d-...`) | Audeze Maxwell (4 slots) | **Android** (`BleDspTransport` nativo) · **Desktop Core** (codecs + `trivium-cli ble-protocols`) | 10 bandas · ±12 dB · Paquete 193B LE | 🧪 Implementado por protocolo (`devicePEQ`) + pruebas unitarias · **Abierto a pruebas comunitarias** |
| **Bluetooth Clásico SPP** (`RFCOMM`) | Moondrop Space Travel 2, Golden Ages 2, Moca, Edge, Nekocake, Robin, BLOCK, Alice | **Linux** (`AF_BLUETOOTH`) + **Android** (`BluetoothSocket`) | 5 bandas · Preamp `-max_pos` · Cambio de preset interno | ✅ **Verificado en hardware real (Golden Ages 2)** |

---

## 3. EQ Universal del Sistema y Exportadores

| Plataforma / Destino | Motor | Capacidades clave | Estado |
| :--- | :--- | :--- | :--- |
| **Linux** (cualquier BT, USB o 3.5mm) | PipeWire `filter-chain` + `trivium-daemon` | 10 bandas · Preamp automático/manual · Ganancia/Atenuación de salida `-30..+12 dB` · Reglas udev `0660+uaccess` | **Verificado en Arch Linux + Hyprland + KDE Plasma 6** |
| **Windows 10/11** (cualquier BT, USB o 3.5mm) | Equalizer APO (`trivium_eq.txt`) | 10 bandas · Preamp + Atenuación/Boost de salida `-30..+12 dB` · GUI nativa sin consolas + `Trivium-Setup-x64.exe` | Compilación y flujos verificados en Windows 11 CI |
| **Android 9+** (cualquier BT, USB o 3.5mm) | `DynamicsProcessing` 32B + Limiter postGain | Curva 32B muestreada · Preamp entrada `-18..+6 dB` · **Atenuación/Amplificación salida `-30..+12 dB`** · Widget home | APK firmado + pruebas JUnit en verde |
| **macOS ARM64** (*Experimental*) | HID / USB-CDC + Exportadores | Compilación nativa `aarch64-apple-darwin` vía `hidapi` (`macos-shared-device`), `hid-scan`, AutoEQ y exportación | Compilado en CI (`macos-14`) · **Sin firma Apple Developer** |
| **Exportadores offline** | Archivos multi-formato | Equalizer APO `.txt`, EasyEffects `.json`, CamillaDSP `.yaml`, Wavelet GraphicEQ 127 bandas | Verificado por pruebas unitarias |
