# Trivium para Android (`com.speed17x.trivium`)

Aplicación nativa de Android escrita en **Kotlin + Jetpack Compose** con temas modulares (**`Default`**, **`Default claro`** y **`NothingOS`** opcional), color de resaltado seleccionable, tipografía Consolas y widget de pantalla de inicio sincronizado.

## Características
- **Pestaña EQ**: gráfica interactiva (`InteractiveEqCurveCanvas.kt`), comparación instantánea **A / B**, control de **Preamp + medidor visual de margen anti-clipping**, **Ganancia / Atenuación de Salida (`-30.0` a `+12.0 dB` vía `DynamicsProcessing.Limiter.postGain`)** para IEMs sensibles, edición de frecuencia, ganancia y factor Q para filtros `PK`, `LS` y `HS`, más botón de apagado completo.
- **Pestaña AutoEQ**: buscador multi-fuente (`AutoEqRepository.kt`, +14,500 curvas) con curvas target y modo de imitación de audífonos.
- **Pestaña Avanzado**: temas modulares y color de acento, presets guardados localmente (`AppPreferencesStore.kt`), importación EqualizerAPO, exportación multi-formato (**APO**, **EasyEffects**, **CamillaDSP** y **Wavelet**), y gestión de transporte USB/BLE/Bluetooth.
- **Widget de Escritorio (`NothingEqWidgetProvider.kt`)**: hereda el tema y color de resaltado elegidos (`Default`, `Default claro` o `NothingOS` con matriz de puntos), con curva en vivo y cambio rápido de presets desde el launcher.
- **Transportes de hardware y sistema**:
  - `UsbMoondropTransport.kt`: comunicación directa USB-C HID/CDC con las **9 familias DSP** (KTMicro 5B, Moondrop 8B, Conexant 9B FreeDSP/ECHO-B, WalkPlay 5–10B, FiiO/JadeAudio/SNOWSKY 5–10B, Qudelix-5K 10B, Topping DX1 II/E50 II 11B, Fosi DS3 8B y JDS Element IV 12B).
  - `BleDspTransport.kt`: transporte inalámbrico BLE GATT nativo para **FiiO EH11/EH13** (10B ±20 dB) y **Airoha / Audeze Maxwell** (10B).
  - `BluetoothMoondropTransport.kt`: control Bluetooth Classic SPP RFCOMM.
  - `GlobalEqService.kt`: servicio en primer plano (`DynamicsProcessing` de 32 bandas) con preamp entrada y limitador postGain configurable (`-30..+12 dB`).

## Requisitos de compilación
- **JDK 17**
- **Android SDK** (`compileSdk = 36`, `minSdk = 28`, `targetSdk = 36`)

## Compilar e instalar

1. Crea `android/local.properties` apuntando a tu Android SDK (no se sube a Git):
   ```properties
   sdk.dir=/ruta/a/tu/Android/Sdk
   ```
2. Compila el APK de depuración o release y ejecuta las pruebas unitarias:
   ```bash
   cd android
   ./gradlew testDebugUnitTest assembleDebug
   ```
3. Instala en un dispositivo conectado por ADB:
   ```bash
   adb install -r app/build/outputs/apk/debug/app-debug.apk
   ```
4. **Actualizaciones automáticas con Obtainium**: importa el perfil [`packaging/android/obtainium-trivium.json`](../packaging/android/obtainium-trivium.json) para recibir actualizaciones firmadas directamente encima desde GitHub Releases.

> **Nota de migración desde AutoQ v2.x (`com.hector.moondropdsp`)**: al cambiar el `applicationId` a **`com.speed17x.trivium`** en Trivium `v3.0.0`, Android trata la aplicación como un paquete distinto y **no actualiza in-place** sobre el APK antiguo de AutoQ. Si vienes de AutoQ v2.x: exporta tus presets si deseas conservarlos, desinstala AutoQ (o mantenlas instaladas en paralelo) e importa [`packaging/android/obtainium-trivium.json`](../packaging/android/obtainium-trivium.json) como una entrada nueva en Obtainium.

## Estructura principal (`app/src/main/kotlin/com/speed17x/trivium/`)
- `dsp/BiquadMath.kt` — Cálculo de coeficientes biquad RBJ, headroom anti-clipping y optimizador PEQ.
- `hardware/MoondropProtocolCodec.kt` — Codificador/decodificador puro JVM para todos los protocolos USB/BLE/BT DSP.
- `hardware/UsbMoondropTransport.kt` — Transporte Android `UsbManager` / HID / USB-CDC.
- `hardware/BleDspTransport.kt` — Transporte BLE GATT (`BluetoothGatt`) para FiiO EH11/EH13 y Airoha/Maxwell.
- `hardware/BluetoothMoondropTransport.kt` — Transporte Bluetooth SPP.
- `hardware/GlobalEqService.kt` — Servicio global de ecualización en Android con preamp y ganancia/atenuación de salida `-30..+12 dB`.
- `data/AutoEqRepository.kt` — Cliente, caché de bases AutoEq + Squiglink y exportadores multi-formato.
- `ui/` — Pantallas Compose (`EqScreen`, `AutoEqScreen`, `AdvancedScreen`), temas modulares y `MoondropViewModel`.
- `widget/` — Proveedor y renderizador Canvas temático del widget de pantalla de inicio.
