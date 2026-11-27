# Protocolo USB-C HID (KTMicro 5-band, Moondrop 8-band y Conexant 9-band)

Trivium implementa los protocolos binarios USB HID directamente en Rust (`crates/trivium-core/src/hid.rs`, `/dev/hidrawX` en Linux y `hidapi` en Windows/macOS) y Kotlin en Android (`MoondropProtocolCodec.kt` + `UsbMoondropTransport.kt`).

---

## 1. Protocolo KTMicro 5 bandas (`VID: 0x31B2`)

Utilizado en **Moondrop Chu 2 DSP (`PID: 0x0113`)**, **CDSP**, **Quark 2** y derivados basados en el controlador KTMicro.

### Formato del paquete (11 bytes)
| Offset | Tamaño | Campo | Valores |
| :--- | :--- | :--- | :--- |
| `0` | 1 byte | `Report ID` | `0x4B` (`'K'`) |
| `1` | 1 byte | `Register` | `0x24` (Slot ON/OFF), `0x26 + 2*i` (Banda `i`, `0..4`) |
| `2..4` | 3 bytes | Reservado | `0x00, 0x00, 0x00` |
| `5` | 1 byte | `Command` | `0x52` (`'R'` Leer), `0x57` (`'W'` Escribir RAM), `0x53` (`'S'` Guardar Flash / Commit) |
| `6` | 1 byte | Reservado | `0x00` |
| `7..10` | 4 bytes | `Payload` | Little-Endian (`int16`/`uint16`) |

### Codificación de cada banda (`i = 0..4`)
Cada banda ocupa **dos registros consecutivos** de 4 bytes:
- **Registro `0x26 + 2*i` (Ganancia y Frecuencia)**:
  - Bytes `0..1` (`int16 LE`): `round(gain_db * 10)` (pasos de `0.1 dB`).
  - Bytes `2..3` (`uint16 LE`): `round(frequency_hz)`.
- **Registro `0x27 + 2*i` (Factor Q y Tipo de filtro)**:
  - Bytes `0..1` (`uint16 LE`): `round(Q * 1000)`.
  - Byte `2`: código de filtro (`0` = Peaking, `3` = Low Shelf, `4` = High Shelf).
  - Byte `3`: `0x00`.

### Activación de EQ y guardado en Flash
- **Registro `0x24` (`KT_REG_SLOT`)**: `0x03` activa el ecualizador; `0x02` lo pone en bypass.
- **Guardado permanente (`KT_COMMIT = 0x53`)**: guarda la RAM en la memoria flash no volátil y provoca un **reinicio rápido de la conexión USB** del dispositivo.

---

## 2. Protocolo Moondrop 8 bandas

Utilizado en **Moondrop FreeDSP Pro / Mini**, **Rays**, **AG Rays**, **Marigold**, **DAWN PRO 2**, **Echo A**, **MOONRIVER 3** y **DHA15**.

### Constantes principales
- `Report ID`: `0x4B`
- `MD_CMD_WRITE`: `0x01` / `MD_CMD_READ`: `0x80`
- `MD_CMD_UPDATE_EQ`: `0x09`
- `MD_CMD_APPLY_COEFF`: `0x0A`
- `MD_CMD_SAVE_FLASH`: `0x01`
- `MD_CMD_ACTIVE_EQ`: `0x0F`
- Códigos de filtro: `1` = Low Shelf, `2` = Peaking, `3` = High Shelf.

---

## 3. Reglas `udev` recomendadas en Linux

Trivium instala automáticamente [`packaging/udev/99-trivium-usb-dsp.rules`](../packaging/udev/99-trivium-usb-dsp.rules) con permisos seguros de sesión local (`MODE="0660", TAG+="uaccess"`) en `/etc/udev/rules.d/99-trivium-usb-dsp.rules` (evita `0666` abierto a cualquier usuario):

```udev
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="31b2", MODE="0660", TAG+="uaccess"
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="2fc6", MODE="0660", TAG+="uaccess"
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="2f20", MODE="0660", TAG+="uaccess"
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="35d8", MODE="0660", TAG+="uaccess"
```

Aplica las reglas sin reiniciar:
```bash
sudo udevadm control --reload-rules && sudo udevadm trigger
```
