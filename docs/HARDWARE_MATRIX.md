# Hardware & Protocol Matrix — **Trivium (`v3.0.0`)**

Compatibility status across USB-C HID/CDC families, wireless BLE GATT transport, Bluetooth Classic SPP, and system-wide EQ engines.

> **Status Legend:** ✅ **Tested on real hardware** · 🧪 **Protocol-implemented + unit tests (open for community testing)**
>
> **How to help expand this list:** plug in your DAC, DSP cable, or headphone and run:
> ```bash
> trivium-cli hid-scan --dump-descriptors --json
> ```
> Open an issue at [Speed17x/Trivium](https://github.com/Speed17x/Trivium/issues) pasting the JSON output (includes `vid:pid`, `usage_page`, and raw descriptor hex without modifying the device).

---

## 1. USB-C HID / Serial Hardware (9 Families)

| Family / Chipset | Key Models | Bands | Filters | Range | Flash | Verification Status |
| :--- | :--- | :---: | :--- | :---: | :---: | :--- |
| **KTMicro** (`VID 0x31B2`) | Moondrop Chu 2 DSP (`0x0113`), May, Quark 2, Jiu, Kiwi Ears Allegro Mini/PRO, JCALLY JM12, Tanchjim Zero/One/Tanya DSP | 5 | PK / LS / HS | ±12 dB | Yes (`0x53`, reconnects) | ✅ **Verified on real hardware (Chu 2 DSP)** + unit tests |
| **Moondrop 8-Band** | FreeDSP Pro, FreeDSP Mini, Rays, AG Rays, Marigold, DAWN PRO 2, Echo A, MOONRIVER 3, DHA15 | 8 | PK / LS / HS (model-dependent) | ±6 / ±12 dB | Yes (`0x01`) | 🧪 Protocol-implemented (`devicePEQ`) + unit tests · **Open for community testing** |
| **Conexant Freeman** (`VID 0x35D8`) | Moondrop FreeDSP (`0x1496`), Moondrop ECHO-B (`0x149B`) | 9 (fixed) | PK / LS / HS | ±12 dB | Direct (4 sample rates) | 🧪 Protocol-implemented (`devicePEQ`) + unit tests · Write-only |
| **WalkPlay** (`19 VIDs`) | Tanchjim Bunny / Stargate II / Space Pro, CrinEar Protocol Max (`10B`), NiceHCK, LETSHUOER, ddHifi, Truthear | 5–10 | PK / LS / HS | ±10 / ±12 dB | Yes (`0x01 0x01`) | 🧪 Protocol-implemented (`devicePEQ`) + unit tests · **Open for community testing** |
| **FiiO / JadeAudio / SNOWSKY** (`0x2972`, `0x0A12`) | JA11 (`5B`), JIEZI (`5B`), KA15, KA17, BTR13, BTR17, QX13, Q7, BT11, SNOWSKY Melody / TINY | 5–10 | PK / LS / HS | ±12 dB | Yes (`0x19` / `0x21`) | 🧪 Protocol-implemented (`devicePEQ`) + unit tests · **Open for community testing** |
| **Qudelix-5K / 5K+** (`VID 0x0A12`) | Qudelix-5K / 5K+ | 10 | PK / LS / HS | ±12 dB | Yes (`0x0708`) | 🧪 Protocol-implemented (V3 bitstream + `SetEqBandParam`) + unit tests · **Open for community testing** |
| **Topping PEQ** (`VID 0x152A`) | Topping DX1 II, E50 II, D50 III, DX5 II | 11 | PK / LS / HS | ±12 dB | Preview (`0x111B`) | 🧪 Protocol-implemented (CRC-16/MODBUS) + unit tests · **Open for community testing** |
| **Fosi Audio** (`VID 0x152A`) | Fosi Audio DS3, ZH3, K7 | 8 | PK / LS / HS | ±12 dB | Yes (`0x92`) | 🧪 Protocol-implemented (`0x77`) + unit tests · **Open for community testing** |
| **JDS Labs USB-CDC** (`0x152A:0x88FA`) | JDS Labs Element IV | 12 | LS + 10 PK + HS | ±12 dB | Immediate (JSON) | 🧪 Protocol-implemented (Serial JSON `Describe`/`Update`) + unit tests · **Open for community testing** |

---

## 2. Wireless BLE GATT & Bluetooth Classic SPP

| Protocol | Compatible Models | Platforms | Capabilities | Status |
| :--- | :--- | :--- | :--- | :--- |
| **FiiO BLE GATT** (`00001100-...`) | FiiO EH11, FiiO EH13 | **Android** (native `BleDspTransport`) · **Desktop Core** (codecs + `trivium-cli ble-protocols`) | 10 bands · ±20 dB · GATT RX/TX read & write | 🧪 Protocol-implemented (`devicePEQ`) + unit tests · **Open for community testing** |
| **Airoha BLE GATT** (`5052494d-...`) | Audeze Maxwell (4 slots) | **Android** (native `BleDspTransport`) · **Desktop Core** (codecs + `trivium-cli ble-protocols`) | 10 bands · ±12 dB · 193B LE frame | 🧪 Protocol-implemented (`devicePEQ`) + unit tests · **Open for community testing** |
| **Bluetooth Classic SPP** (`RFCOMM`) | Moondrop Space Travel 2, Golden Ages 2, Moca, Edge, Nekocake, Robin, BLOCK, Alice | **Linux** (`AF_BLUETOOTH`) + **Android** (`BluetoothSocket`) | 5 bands · `-max_pos` preamp · Built-in preset switch | 🧪 Protocol-implemented + unit tests · **Open for community testing** |

---

## 3. System-Wide EQ & Offline Exporters

| Platform / Target | Engine | Key Capabilities | Status |
| :--- | :--- | :--- | :--- |
| **Linux** (any BT, USB, or 3.5mm) | PipeWire `filter-chain` + `trivium-daemon` | 10 bands · Auto/manual preamp · Output Gain/Attenuation `-30..+12 dB` · `0660+uaccess` udev rules | ✅ **Verified on Arch Linux + Hyprland + KDE Plasma 6** |
| **Windows 10/11** (any BT, USB, or 3.5mm) | Equalizer APO (`trivium_eq.txt`) | 10 bands · Preamp + `-30..+12 dB` Output Gain/Attenuation · Console-free native GUI + `Trivium-Setup-x64.exe` | Builds & flows verified on Windows 11 CI |
| **Android 9+** (any BT, USB, or 3.5mm) | `DynamicsProcessing` 32B + Limiter postGain | Sampled 32B curve · Input preamp `-18..+6 dB` · **Output Attenuation/Gain `-30..+12 dB`** · Home widget | Signed APK + JUnit tests passing |
| **macOS ARM64** (*Experimental*) | HID / USB-CDC + Exporters | Native `aarch64-apple-darwin` build via `hidapi` (`macos-shared-device`), `hid-scan`, AutoEQ & exporters | Built on CI (`macos-14`) · **No Apple Developer signature** |
| **Offline exporters** | Multi-format files | Equalizer APO `.txt`, EasyEffects `.json`, CamillaDSP `.yaml`, Wavelet GraphicEQ 127-band | Verified by unit tests |
