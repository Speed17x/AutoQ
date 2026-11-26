package com.hector.moondropdsp.hardware

import android.Manifest
import android.annotation.SuppressLint
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothSocket
import android.content.Context
import android.content.pm.PackageManager
import android.media.AudioDeviceInfo
import android.media.AudioManager
import android.os.Build
import androidx.core.content.ContextCompat
import com.hector.moondropdsp.dsp.FilterBand
import java.io.ByteArrayOutputStream
import java.io.IOException
import java.util.UUID

/**
 * Universal audio output detector (all Bluetooth headphones/TWS, generic USB-C DACs/headsets,
 * and 3.5mm wired headphones) plus optional Moondrop Classic Bluetooth SPP RFCOMM transport.
 */
class BluetoothMoondropTransport(private val context: Context) {

    companion object {
        private val SPP_UUID: UUID = UUID.fromString("00001101-0000-1000-8000-00805F9B34FB")
    }

    fun hasBluetoothConnectPermission(): Boolean {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            ContextCompat.checkSelfPermission(
                context,
                Manifest.permission.BLUETOOTH_CONNECT,
            ) == PackageManager.PERMISSION_GRANTED
        } else {
            true
        }
    }

    /**
     * Detects ALL connected external audio outputs (any brand Bluetooth A2DP/BLE,
     * generic USB-C DACs/headsets not already claimed as USB HID, and 3.5mm wired headphones).
     */
    fun scanConnectedExternalAudio(usbHidDevices: List<MoondropDevice> = emptyList()): List<MoondropDevice> {
        val audioManager = context.getSystemService(Context.AUDIO_SERVICE) as? AudioManager ?: return emptyList()
        val outputs = audioManager.getDevices(AudioManager.GET_DEVICES_OUTPUTS)
        val bondedMacByName = lookupBondedMacs()
        val seen = LinkedHashMap<String, MoondropDevice>()

        val hidNamesLower = usbHidDevices.map { it.displayName.lowercase() }

        for (dev in outputs) {
            val isBt = dev.type == AudioDeviceInfo.TYPE_BLUETOOTH_A2DP ||
                dev.type == AudioDeviceInfo.TYPE_HEARING_AID ||
                (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S &&
                    (dev.type == AudioDeviceInfo.TYPE_BLE_HEADSET || dev.type == AudioDeviceInfo.TYPE_BLE_SPEAKER))

            val isUsbAudio = dev.type == AudioDeviceInfo.TYPE_USB_HEADSET ||
                dev.type == AudioDeviceInfo.TYPE_USB_DEVICE

            val isWired = dev.type == AudioDeviceInfo.TYPE_WIRED_HEADSET ||
                dev.type == AudioDeviceInfo.TYPE_WIRED_HEADPHONES

            if (!isBt && !isUsbAudio && !isWired) continue

            val rawName = dev.productName?.toString()?.trim().orEmpty()
            val displayName = when {
                rawName.isNotBlank() && rawName != Build.MODEL -> rawName
                isWired -> "Audífonos 3.5mm"
                isUsbAudio -> "DAC / Audífonos USB-C"
                else -> "Audífonos Bluetooth"
            }

            // Avoid duplicating a USB-C DAC already handled via direct USB HID DSP
            if (isUsbAudio) {
                val low = displayName.lowercase()
                if (hidNamesLower.any { it.contains(low) || low.contains("chu2") || low.contains("chu 2") }) {
                    continue
                }
            }

            if (isBt) {
                val addr = dev.address.takeIf { it.contains(":") && it != "00:00:00:00:00:00" }
                    ?: bondedMacByName[displayName.lowercase()].orEmpty()

                val key = "bt:" + addr.ifBlank { displayName.lowercase() }
                if (seen.containsKey(key)) continue
                seen[key] = MoondropDevice(
                    id = key,
                    kind = DeviceKind.BT_AUDIO,
                    protocol = DspProtocol.SYSTEM_DYNAMICS,
                    displayName = displayName,
                    shortWidgetName = MoondropDeviceCatalog.shortenWidgetName(displayName),
                    transportBadge = "BT·10B",
                    maxBands = 10,
                    maxGainDb = 12.0,
                    supportsShelves = true,
                    disconnectsOnCommit = false,
                    autoeqHint = MoondropDeviceCatalog.inferAutoEqHint(displayName),
                    btMac = addr,
                    hasUsbPermission = true,
                    supportsMoondropSpp = MoondropDeviceCatalog.isMoondropBluetoothName(displayName),
                )
            } else if (isUsbAudio) {
                val key = "usbaudio:${displayName.lowercase()}"
                if (seen.containsKey(key)) continue
                seen[key] = MoondropDevice(
                    id = key,
                    kind = DeviceKind.USB_AUDIO,
                    protocol = DspProtocol.SYSTEM_DYNAMICS,
                    displayName = displayName,
                    shortWidgetName = MoondropDeviceCatalog.shortenWidgetName(displayName),
                    transportBadge = "USB·10B",
                    maxBands = 10,
                    maxGainDb = 12.0,
                    supportsShelves = true,
                    disconnectsOnCommit = false,
                    autoeqHint = MoondropDeviceCatalog.inferAutoEqHint(displayName),
                    hasUsbPermission = true,
                )
            } else {
                val key = "wired:aux"
                if (seen.containsKey(key)) continue
                seen[key] = MoondropDevice(
                    id = key,
                    kind = DeviceKind.WIRED_AUDIO,
                    protocol = DspProtocol.SYSTEM_DYNAMICS,
                    displayName = displayName,
                    shortWidgetName = "AUX 3.5MM",
                    transportBadge = "AUX·10B",
                    maxBands = 10,
                    maxGainDb = 12.0,
                    supportsShelves = true,
                    disconnectsOnCommit = false,
                    autoeqHint = "",
                    hasUsbPermission = true,
                )
            }
        }
        return seen.values.toList()
    }

    fun listAllConnectedAudioOutputs(): List<String> {
        val audioManager = context.getSystemService(Context.AUDIO_SERVICE) as? AudioManager ?: return emptyList()
        return audioManager.getDevices(AudioManager.GET_DEVICES_OUTPUTS)
            .filter {
                it.type == AudioDeviceInfo.TYPE_BLUETOOTH_A2DP ||
                    it.type == AudioDeviceInfo.TYPE_USB_DEVICE ||
                    it.type == AudioDeviceInfo.TYPE_USB_HEADSET ||
                    it.type == AudioDeviceInfo.TYPE_WIRED_HEADSET ||
                    it.type == AudioDeviceInfo.TYPE_WIRED_HEADPHONES ||
                    (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S &&
                        (it.type == AudioDeviceInfo.TYPE_BLE_HEADSET || it.type == AudioDeviceInfo.TYPE_BLE_SPEAKER))
            }
            .map { "${it.productName} (tipo ${it.type})" }
            .distinct()
    }

    @SuppressLint("MissingPermission")
    private fun lookupBondedMacs(): Map<String, String> {
        if (!hasBluetoothConnectPermission()) return emptyMap()
        val btMgr = context.getSystemService(Context.BLUETOOTH_SERVICE) as? BluetoothManager ?: return emptyMap()
        val adapter = btMgr.adapter ?: return emptyMap()
        return try {
            adapter.bondedDevices.orEmpty().associate { dev ->
                (dev.name ?: "").lowercase() to (dev.address ?: "")
            }
        } catch (_: SecurityException) {
            emptyMap()
        }
    }

    @SuppressLint("MissingPermission")
    private fun sppRequest(mac: String, payload: ByteArray, timeoutMs: Long = 2200L): ByteArray {
        if (mac.isBlank()) throw IOException("No se detectó dirección MAC Bluetooth para SPP.")
        if (!hasBluetoothConnectPermission()) {
            throw SecurityException("Se requiere permiso BLUETOOTH_CONNECT para comunicación SPP.")
        }
        val btMgr = context.getSystemService(Context.BLUETOOTH_SERVICE) as? BluetoothManager
            ?: throw IOException("Bluetooth no disponible en este teléfono.")
        val adapter = btMgr.adapter ?: throw IOException("Adaptador Bluetooth no disponible.")
        val remote = adapter.getRemoteDevice(mac)

        var sock: BluetoothSocket? = null
        try {
            sock = try {
                remote.createRfcommSocketToServiceRecord(SPP_UUID)
            } catch (_: Exception) {
                remote.createInsecureRfcommSocketToServiceRecord(SPP_UUID)
            }
            sock.connect()
            val out = sock.outputStream
            val input = sock.inputStream
            out.write(MoondropProtocolCodec.sppFrame(payload))
            out.flush()

            val want = payload[2].toInt() and 0xFE
            val acc = ByteArrayOutputStream()
            val tmp = ByteArray(512)
            val deadline = System.currentTimeMillis() + timeoutMs
            while (System.currentTimeMillis() < deadline) {
                if (input.available() > 0) {
                    val n = input.read(tmp)
                    if (n <= 0) break
                    acc.write(tmp, 0, n)
                    val buf = acc.toByteArray()
                    val frame = extractMatchingSppFrame(buf, want)
                    if (frame != null) return frame
                } else {
                    Thread.sleep(25)
                }
            }
            throw IOException("Tiempo de espera agotado esperando respuesta Bluetooth SPP.")
        } finally {
            try {
                sock?.close()
            } catch (_: Exception) {
            }
        }
    }

    private fun extractMatchingSppFrame(buf: ByteArray, wantCmd: Int): ByteArray? {
        var i = 0
        while (i + 8 <= buf.size) {
            if (buf[i] == 0xFF.toByte() && buf[i + 1] == 0x04.toByte()) {
                val ln = ((buf[i + 2].toInt() and 0xFF) shl 8) or (buf[i + 3].toInt() and 0xFF)
                val total = 8 + ln
                if (buf.size - i >= total) {
                    val frame = buf.copyOfRange(i, i + total)
                    if ((frame[6].toInt() and 0xFE) == wantCmd) {
                        return frame
                    }
                    i += total
                    continue
                }
                return null
            }
            i++
        }
        return null
    }

    fun sppSetPreset(mac: String, presetIdx: Int) {
        sppRequest(mac, MoondropProtocolCodec.sppPresetPayload(presetIdx), timeoutMs = 1800L)
    }

    fun sppReadEq(mac: String): List<FilterBand> {
        val frame = sppRequest(mac, MoondropProtocolCodec.sppReadEqPayload(), timeoutMs = 2400L)
        return MoondropProtocolCodec.sppDecodeEqFrame(frame)
            ?: throw IOException("Este modelo Bluetooth no devolvió una curva PEQ personalizable por SPP.")
    }

    fun sppWriteEq(mac: String, bands: List<FilterBand>) {
        for (slot in intArrayOf(0x03, 0xFF)) {
            try {
                sppRequest(mac, MoondropProtocolCodec.sppPresetPayload(slot), timeoutMs = 900L)
                break
            } catch (_: Exception) {
            }
        }
        sppRequest(mac, MoondropProtocolCodec.sppWriteEqPayload(bands), timeoutMs = 2400L)
    }
}
