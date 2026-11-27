# Evaluación Técnica: Núcleo Compartido `trivium-core` en Android (`JNI` / `UniFFI`)

> Documento de diseño iniciado en `v2.2.0` y actualizado para `v3.0.0` (`Trivium`), que evalúa la unificación futura del código DSP y los 9 códecs binarios USB HID + BLE GATT de `crates/trivium-core` hacia la aplicación Android (`android/`) mediante compilación cruzada a `aarch64-linux-android`.

---

## 1. Motivación actual

Actualmente **Trivium** mantiene dos implementaciones paralelas pero independientes:
- **Rust (`crates/trivium-core`)**: `biquad.rs` (optimizador RBJ), `autoeq.rs` (catálogo + parsers/exportadores), `hid.rs` / `jds_serial.rs` (9 familias USB-C DSP) y `ble_dsp.rs` (BLE GATT).
- **Kotlin (`android/`)**: `BiquadMath.kt`, `AutoEqRepository.kt`, `MoondropProtocolCodec.kt` y `BleDspTransport.kt`.

Ambas suites mantienen paridad funcional estricta y pruebas unitarias espejo, pero cada mejora de protocolo o exportador requiere implementarse en dos lenguajes.

---

## 2. Arquitectura evaluada

| Capa | Opción A: Kotlin espejo (actual en `v3.0.0`) | Opción B: `trivium-core` compilado con `cargo-ndk` + `uniffi` |
| :--- | :--- | :--- |
| **Matemáticas RBJ & AutoEQ** | ~1,200 LOC Kotlin + tests JVM | Fuente única en `crates/trivium-core` (`no-os` DSP puro) |
| **Transporte USB HID** | Kotlin `UsbDeviceConnection.bulkTransfer / controlTransfer` | Kotlin sigue abriendo `UsbManager` (Android bloquea `/dev/hidraw`), pasando el descriptor `conn.fileDescriptor` o manteniendo codificación de paquetes pura en Rust |
| **Tamaño del APK (`arm64-v8a` + `armeabi-v7a`)** | ~6.5 MB APK release | **+2.5 MB por ABI** (~9 MB 1 ABI / ~11.5 MB universal) incluyendo `ureq`/`rustls` (o **+0.6 MB** si `trivium-ffi` solo expone `biquad` + `codec` + parsers sin duplicar cliente HTTP `ureq`) |
| **Complejidad de CI/CD** | JDK 17 + Android SDK en GitHub Actions | Requiere `rustup target add aarch64-linux-android armv7-linux-androideabi`, Android NDK instalado en CI y generación de bindings `.kt` |

---

## 3. Hallazgos clave sobre USB-C HID en Android

En Android sin *root*, `libhidapi` no puede enumerar `/dev/bus/usb` ni `/dev/hidraw*` directamente debido a políticas SELinux y al modelo de permisos por `UsbManager.requestPermission(...)`.
Por tanto:
1. La **enumeración de dispositivos, solicitud de permiso USB y transferencia por endpoints (`controlTransfer` / `bulkTransfer`)** DEBE permanecer en Kotlin (`UsbMoondropTransport.kt`), al igual que el cliente `BluetoothGatt` (`BleDspTransport.kt`).
2. Lo que sí conviene compartir desde Rust sin fricción ni duplicar TLS (`rustls` ya vive en OkHttp/HttpURLConnection en Android) es un crate ligero `trivium-dsp-codec` (sin `ureq`, `libc` ni `winreg`) compilado vía **UniFFI** o **JNI mínimo** solo para:
   - `fit_peq_bands` / `combined_response_db` / `peak_boost_db` / `headroom_margin_db`
   - Codificación/decodificación de paquetes (`Vec<u8>`) de las 9 familias USB DSP y los códecs BLE GATT
   - Exportadores (`ParametricEQ`, `EasyEffects`, `CamillaDSP`, `Wavelet`) y parsers REW/Squiglink.

---

## 4. Veredicto recomendado

1. **Mantener la división pura en `v3.0.0`** hasta reunir capturas reales de la matriz pública [`HARDWARE_MATRIX.md`](./HARDWARE_MATRIX.md) (Windows 10/11, macOS ARM64 y firmwares BLE GATT / Conexant 9B), evitando añadir Android NDK y `cargo-ndk` al pipeline de release antes de cerrar la validación física.
2. **Siguiente paso natural (`v3.x`)**: extraer un sub-crate sin dependencias de red/OS (`trivium-dsp-codec`) dentro del workspace Cargo existente para generar `libtrivium_ffi.so` (~600 KB) con `uniffi` en Android sin duplicar `ureq`/`rustls` ni romper el flujo nativo de `UsbManager` / `BluetoothGatt`.
