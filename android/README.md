# AutoQ para Android

Aplicación nativa de Android escrita en **Kotlin + Jetpack Compose** con estética **Nothing OS** y widget de pantalla de inicio (`AutoQ · Nothing`).

## Características
- **Pestaña EQ**: gráfica interactiva (`InteractiveEqCurveCanvas.kt`) con respuesta combinada en tiempo real, edición de frecuencia, ganancia y factor Q para filtros `PK`, `LS` y `HS`.
- **Pestaña AutoEQ**: buscador multi-fuente (`AutoEqRepository.kt`) con curvas target y modo de imitación de audífonos.
- **Pestaña Avanzado**: presets guardados localmente (`AppPreferencesStore.kt`), importación/exportación EqualizerAPO, y gestión de transporte USB/Bluetooth.
- **Widget Nothing OS**: renderizado matricial (`NothingDotMatrixRenderer.kt`, `NothingEqWidgetProvider.kt`) con curva en vivo y cambio rápido de presets desde el launcher.
- **Transportes de hardware y sistema**:
  - `UsbMoondropTransport.kt`: comunicación directa USB-C HID con chips KTMicro (5 bandas) y Moondrop (8 bandas).
  - `BluetoothMoondropTransport.kt`: control Bluetooth Classic SPP RFCOMM.
  - `GlobalEqService.kt`: servicio en primer plano para ecualización global en cualquier audífono conectado.

## Requisitos de compilación
- **JDK 17**
- **Android SDK** (`compileSdk = 36`, `minSdk = 28`, `targetSdk = 36`)

## Compilar e instalar

1. Crea `android/local.properties` apuntando a tu Android SDK (no se sube a Git):
   ```properties
   sdk.dir=/ruta/a/tu/Android/Sdk
   ```
2. Compila el APK de depuración o release:
   ```bash
   cd android
   ./gradlew assembleDebug
   ```
3. Ejecuta las pruebas unitarias del motor DSP y códec de protocolo:
   ```bash
   ./gradlew testDebugUnitTest
   ```
4. Instala en un dispositivo conectado por ADB:
   ```bash
   adb install -r app/build/outputs/apk/debug/app-debug.apk
   ```

## Estructura principal (`app/src/main/kotlin/com/hector/moondropdsp/`)
- `dsp/BiquadMath.kt` — Cálculo de coeficientes biquad RBJ, respuesta en frecuencia y ajuste óptimo de bandas PEQ.
- `hardware/MoondropProtocolCodec.kt` — Codificador/decodificador puro JVM para paquetes HID KTMicro y Moondrop 8-band.
- `hardware/UsbMoondropTransport.kt` — Transporte Android `UsbManager` / HID.
- `hardware/BluetoothMoondropTransport.kt` — Transporte Bluetooth SPP.
- `hardware/GlobalEqService.kt` — Servicio global de ecualización en Android.
- `data/AutoEqRepository.kt` — Cliente y caché de bases AutoEq + Squiglink.
- `ui/` — Pantallas Compose (`EqScreen`, `AutoEqScreen`, `AdvancedScreen`), tema Nothing OS y `MoondropViewModel`.
- `widget/` — Proveedor y renderizador del widget de escritorio Android.
