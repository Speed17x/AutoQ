# Evaluación Técnica: Núcleo Compartido `autoq-core` en Android (`JNI` / `UniFFI`)

> Documento de diseño correspondiente al ciclo `v2.2.0` para decidir si unificar el código DSP y los 8 códecs binarios de `crates/autoq-core` hacia la aplicación Android (`android/`) mediante compilación cruzada a `aarch64-linux-android`.

---

## 1. Motivación actual

Actualmente **AutoQ** mantiene dos implementaciones paralelas pero independientes:
- **Rust (`crates/autoq-core`)**: `biquad.rs` (optimizador RBJ), `autoeq.rs` (catálogo + parsers/exportadores) y `hid.rs` / `jds_serial.rs` (8 familias DSP).
- **Kotlin (`android/`)**: `BiquadMath.kt`, `AutoEqRepository.kt` y `MoondropProtocolCodec.kt`.

Ambas suites mantienen paridad funcional estricta y pruebas unitarias espejo, pero cada mejora de protocolo o exportador requiere implementarse en dos lenguajes.

---

## 2. Arquitectura evaluada

| Capa | Opción A: Kotlin espejo (actual) | Opción B: `autoq-core` compilado con `cargo-ndk` + `uniffi` |
| :--- | :--- | :--- |
| **Matemáticas RBJ & AutoEQ** | ~1,200 LOC Kotlin + tests JVM | Fuente única en `crates/autoq-core` (`no-os` DSP puro) |
| **Transporte USB HID** | Kotlin `UsbDeviceConnection.bulkTransfer / controlTransfer` | Kotlin sigue abriendo `UsbManager` (Android bloquea `/dev/hidraw`), pasando el descriptor `conn.fileDescriptor` o manteniendo codificación de paquetes pura en Rust |
| **Tamaño del APK (`arm64-v8a` + `armeabi-v7a`)** | ~6.5 MB APK release | **+2.5 MB por ABI** (~9 MB 1 ABI / ~11.5 MB universal) incluyendo `ureq`/`rustls` (o **+0.6 MB** si `autoq-ffi` solo expone `biquad` + `codec` + parsers sin duplicar cliente HTTP `ureq`) |
| **Complejidad de CI/CD** | JDK 17 + Android SDK en GitHub Actions | Requiere `rustup target add aarch64-linux-android armv7-linux-androideabi`, Android NDK instalado en CI y generación de bindings `.kt` |

---

## 3. Hallazgos clave sobre USB-C HID en Android

En Android sin *root*, `libhidapi` no puede enumerar `/dev/bus/usb` ni `/dev/hidraw*` directamente debido a políticas SELinux y al modelo de permisos por `UsbManager.requestPermission(...)`.
Por tanto:
1. La **enumeración de dispositivos, solicitud de permiso USB y transferencia por endpoints (`controlTransfer` / `bulkTransfer`)** DEBE permanecer en Kotlin (`UsbMoondropTransport.kt`).
2. Lo que sí conviene compartir desde Rust sin fricción ni duplicar TLS (`rustls` ya vive en OkHttp/HttpURLConnection en Android) es un crate ligero `autoq-dsp-pure` (sin `ureq`, `libc` ni `winreg`) compilado vía **UniFFI** o **JNI mínimo** solo para:
   - `fit_peq_bands` / `combined_response_db` / `peak_boost_db` / `headroom_margin_db`
   - Codificación/decodificación de paquetes (`Vec<u8>`) de las 8 familias DSP
   - Exportadores (`ParametricEQ`, `EasyEffects`, `CamillaDSP`, `Wavelet`) y parsers REW/Squiglink.

---

## 4. Veredicto recomendado

1. **Mantener la división pura por ahora en `v2.2.0`** hasta tener pruebas físicas completas de Windows 10/11 y firmware de las 8 familias (Fase 6), evitando añadir Android NDK y `cargo-ndk` al pipeline de release antes de estabilizar los últimos firmwares.
2. **Siguiente paso natural (`v3.x`)**: extraer un sub-crate sin dependencias de red/OS (`autoq-dsp-codec`) dentro del workspace Cargo existente para generar `libautoq_ffi.so` (~600 KB) con `uniffi` en Android sin duplicar `ureq`/`rustls` ni romper el flujo nativo de `UsbManager`.
