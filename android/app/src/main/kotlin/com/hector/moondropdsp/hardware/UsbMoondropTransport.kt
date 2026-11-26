package com.hector.moondropdsp.hardware

import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.hardware.usb.UsbConstants
import android.hardware.usb.UsbDevice
import android.hardware.usb.UsbDeviceConnection
import android.hardware.usb.UsbEndpoint
import android.hardware.usb.UsbInterface
import android.hardware.usb.UsbManager
import android.os.Build
import com.hector.moondropdsp.dsp.FilterBand
import com.hector.moondropdsp.dsp.FilterType

class MoondropUsbException(message: String, cause: Throwable? = null) : RuntimeException(message, cause)

data class UsbDiagnosticEntry(
    val deviceName: String,
    val productName: String,
    val vidPid: String,
    val interfaceCount: Int,
    val hasHidInterface: Boolean,
    val hasPermission: Boolean,
    val recognizedAs: String?,
)

class UsbMoondropTransport(private val context: Context) {

    companion object {
        const val ACTION_USB_PERMISSION = "com.hector.moondropdsp.USB_PERMISSION"
        private const val HID_REQUEST_TYPE_OUT = 0x21 // Host->Device | Class | Interface
        private const val HID_SET_REPORT = 0x09
        private const val HID_REPORT_TYPE_OUTPUT = 0x02
        private val usbSessionLock = Any()
    }

    private val usbManager: UsbManager?
        get() = context.getSystemService(Context.USB_SERVICE) as? UsbManager

    fun scanDevices(): List<MoondropDevice> {
        val mgr = usbManager ?: return emptyList()
        val found = mutableListOf<MoondropDevice>()
        for (usbDev in mgr.deviceList.values) {
            val product = safeProductName(usbDev)
            val info = MoondropDeviceCatalog.classifyUsb(
                deviceName = usbDev.deviceName,
                vid = usbDev.vendorId,
                pid = usbDev.productId,
                productName = product,
                hasPermission = mgr.hasPermission(usbDev),
            )
            if (info != null) {
                found += info
            }
        }
        return found.sortedBy { it.usbDeviceName }
    }

    fun inspectAllUsbDevices(): List<UsbDiagnosticEntry> {
        val mgr = usbManager ?: return emptyList()
        return mgr.deviceList.values.map { d ->
            val product = safeProductName(d)
            val classified = MoondropDeviceCatalog.classifyUsb(
                deviceName = d.deviceName,
                vid = d.vendorId,
                pid = d.productId,
                productName = product,
                hasPermission = mgr.hasPermission(d),
            )
            val hasHid = (0 until d.interfaceCount).any {
                d.getInterface(it).interfaceClass == UsbConstants.USB_CLASS_HID
            }
            UsbDiagnosticEntry(
                deviceName = d.deviceName,
                productName = product.ifBlank { "USB ${d.vendorId.toString(16)}:${d.productId.toString(16)}" },
                vidPid = String.format(java.util.Locale.US, "%04X:%04X", d.vendorId, d.productId),
                interfaceCount = d.interfaceCount,
                hasHidInterface = hasHid,
                hasPermission = mgr.hasPermission(d),
                recognizedAs = classified?.displayName,
            )
        }
    }

    fun findUsbDevice(info: MoondropDevice): UsbDevice? {
        val mgr = usbManager ?: return null
        return mgr.deviceList[info.usbDeviceName]
            ?: mgr.deviceList.values.firstOrNull { it.vendorId == info.vid && it.productId == info.pid }
    }

    fun requestPermission(info: MoondropDevice): Boolean {
        val mgr = usbManager ?: return false
        val usbDev = findUsbDevice(info) ?: return false
        if (mgr.hasPermission(usbDev)) return true
        val intent = Intent(ACTION_USB_PERMISSION).apply {
            setPackage(context.packageName)
        }
        val flags = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_MUTABLE
        } else {
            PendingIntent.FLAG_UPDATE_CURRENT
        }
        val pi = PendingIntent.getBroadcast(context, usbDev.deviceId, intent, flags)
        mgr.requestPermission(usbDev, pi)
        return false
    }

    fun readEq(info: MoondropDevice): Pair<Boolean, List<FilterBand>> = withHidSession(info) { session ->
        when (info.protocol) {
            DspProtocol.KTMICRO_5B -> {
                val slotReply = session.ktCommand(
                    reg = MoondropProtocolCodec.KT_REG_SLOT,
                    cmd = MoondropProtocolCodec.KT_READ,
                    value = byteArrayOf(MoondropProtocolCodec.KT_SLOT_ON, 0, 0, 0),
                )
                val enabled = slotReply[0] != MoondropProtocolCodec.KT_SLOT_OFF
                val bands = MutableList(info.maxBands) { i ->
                    val reg = (MoondropProtocolCodec.KT_REG_BAND0.toInt() + 2 * i).toByte()
                    val gf = session.ktCommand(reg, MoondropProtocolCodec.KT_READ)
                    val qt = session.ktCommand((reg + 1).toByte(), MoondropProtocolCodec.KT_READ)
                    MoondropProtocolCodec.ktDecodeBand(gf, qt)
                }
                enabled to bands
            }

            DspProtocol.MOONDROP_8B -> {
                val bands = MutableList(info.maxBands) { i ->
                    val req = MoondropProtocolCodec.md8BuildReadPacket(i)
                    val reply = session.md8SendAndRead(req)
                    MoondropProtocolCodec.md8DecodeReadReply(reply)
                        ?: FilterBand(FilterType.PEAKING, 1000.0, 0.0, 1.0)
                }
                true to bands
            }

            DspProtocol.SYSTEM_DYNAMICS -> throw MoondropUsbException("Este dispositivo usa EQ del sistema (no USB HID).")
        }
    }

    fun writeEq(
        info: MoondropDevice,
        bands: List<FilterBand>,
        commit: Boolean = false,
        enableEq: Boolean = true,
    ) = withHidSession(info) { session ->
        val fitted = bands.take(info.maxBands).map {
            it.clamped(maxGainDb = info.maxGainDb, allowShelves = info.supportsShelves)
        }.toMutableList()
        while (fitted.size < info.maxBands) {
            fitted += FilterBand(FilterType.PEAKING, 1000.0, 0.0, 1.0)
        }

        when (info.protocol) {
            DspProtocol.KTMICRO_5B -> {
                val slot = session.ktCommand(
                    reg = MoondropProtocolCodec.KT_REG_SLOT,
                    cmd = MoondropProtocolCodec.KT_READ,
                    value = byteArrayOf(MoondropProtocolCodec.KT_SLOT_ON, 0, 0, 0),
                )[0]
                if (enableEq && slot == MoondropProtocolCodec.KT_SLOT_OFF) {
                    session.ktCommand(
                        MoondropProtocolCodec.KT_REG_SLOT,
                        MoondropProtocolCodec.KT_WRITE,
                        byteArrayOf(MoondropProtocolCodec.KT_SLOT_ON, 0, 0, 0),
                    )
                } else if (!enableEq && slot != MoondropProtocolCodec.KT_SLOT_OFF) {
                    session.ktCommand(
                        MoondropProtocolCodec.KT_REG_SLOT,
                        MoondropProtocolCodec.KT_WRITE,
                        byteArrayOf(MoondropProtocolCodec.KT_SLOT_OFF, 0, 0, 0),
                    )
                }

                for (i in fitted.indices) {
                    val reg = (MoondropProtocolCodec.KT_REG_BAND0.toInt() + 2 * i).toByte()
                    val (gf, qt) = MoondropProtocolCodec.ktEncodeBand(fitted[i])
                    session.ktCommand(reg, MoondropProtocolCodec.KT_WRITE, gf)
                    session.ktCommand((reg + 1).toByte(), MoondropProtocolCodec.KT_WRITE, qt)
                    val readGf = session.ktCommand(reg, MoondropProtocolCodec.KT_READ)
                    val readQt = session.ktCommand((reg + 1).toByte(), MoondropProtocolCodec.KT_READ)
                    if (!readGf.contentEquals(gf) || !readQt.contentEquals(qt)) {
                        throw MoondropUsbException(
                            "Verificación fallida en banda ${i + 1}: el DSP devolvió datos distintos. No se guardó en flash."
                        )
                    }
                }

                if (commit) {
                    session.ktCommand(
                        reg = 0x00,
                        cmd = MoondropProtocolCodec.KT_COMMIT,
                        ignoreDisconnect = true,
                    )
                }
            }

            DspProtocol.MOONDROP_8B -> {
                for (i in fitted.indices) {
                    session.sendHidReport(MoondropProtocolCodec.md8BuildWritePacket(i, fitted[i]))
                    session.sendHidReport(MoondropProtocolCodec.md8BuildApplyPacket(i))
                    Thread.sleep(15)
                }
                if (commit) {
                    session.sendHidReport(MoondropProtocolCodec.md8BuildSaveFlashPacket())
                }
            }

            DspProtocol.SYSTEM_DYNAMICS -> throw MoondropUsbException("Este dispositivo usa EQ del sistema (no USB HID).")
        }
    }

    private fun <T> withHidSession(info: MoondropDevice, block: (HidSession) -> T): T = synchronized(usbSessionLock) {
        val mgr = usbManager ?: throw MoondropUsbException("Servicio USB no disponible en este dispositivo.")
        val usbDev = findUsbDevice(info)
            ?: throw MoondropUsbException("${info.displayName} no está conectado por USB-C.")
        if (!mgr.hasPermission(usbDev)) {
            throw MoondropUsbException("Sin permiso USB para ${info.displayName}. Pulsa 'Autorizar USB'.")
        }

        val hidIface = findHidInterface(usbDev)
            ?: throw MoondropUsbException("No se encontró interfaz HID en ${info.displayName} (${info.vidPid}).")

        val conn = mgr.openDevice(usbDev)
            ?: throw MoondropUsbException("No se pudo abrir la conexión USB con ${info.displayName}.")

        if (!conn.claimInterface(hidIface, true)) {
            conn.close()
            throw MoondropUsbException("No se pudo reclamar la interfaz HID de ${info.displayName}.")
        }

        return try {
            val session = HidSession(conn, hidIface)
            block(session)
        } finally {
            try {
                conn.releaseInterface(hidIface)
            } catch (_: Exception) {
            }
            try {
                conn.close()
            } catch (_: Exception) {
            }
        }
    }

    private fun findHidInterface(device: UsbDevice): UsbInterface? {
        val hidInterfaces = (0 until device.interfaceCount)
            .map { device.getInterface(it) }
            .filter { it.interfaceClass == UsbConstants.USB_CLASS_HID }
        // Prefer HID interface that has both IN and OUT endpoints, otherwise any HID interface with IN
        return hidInterfaces.firstOrNull { iface ->
            val eps = (0 until iface.endpointCount).map { iface.getEndpoint(it) }
            eps.any { it.direction == UsbConstants.USB_DIR_IN } && eps.any { it.direction == UsbConstants.USB_DIR_OUT }
        } ?: hidInterfaces.firstOrNull { iface ->
            (0 until iface.endpointCount).any { iface.getEndpoint(it).direction == UsbConstants.USB_DIR_IN }
        } ?: hidInterfaces.firstOrNull()
    }

    private fun safeProductName(device: UsbDevice): String = try {
        device.productName ?: device.manufacturerName ?: ""
    } catch (_: SecurityException) {
        ""
    }

    private class HidSession(
        private val conn: UsbDeviceConnection,
        private val iface: UsbInterface,
    ) {
        private val epIn: UsbEndpoint?
        private val epOut: UsbEndpoint?

        init {
            var inEp: UsbEndpoint? = null
            var outEp: UsbEndpoint? = null
            for (i in 0 until iface.endpointCount) {
                val ep = iface.getEndpoint(i)
                if (ep.direction == UsbConstants.USB_DIR_IN && inEp == null) inEp = ep
                if (ep.direction == UsbConstants.USB_DIR_OUT && outEp == null) outEp = ep
            }
            epIn = inEp
            epOut = outEp
        }

        private fun drainInput() {
            val ep = epIn ?: return
            val buf = ByteArray(maxOf(64, ep.maxPacketSize))
            repeat(6) {
                val n = conn.bulkTransfer(ep, buf, buf.size, 10)
                if (n <= 0) return
            }
        }

        fun sendHidReport(packet: ByteArray) {
            val out = epOut
            if (out != null) {
                val sendLen = if (out.maxPacketSize in packet.size..64) out.maxPacketSize else packet.size
                val buf = if (sendLen == packet.size) packet else packet.copyOf(sendLen)
                val written = conn.bulkTransfer(out, buf, buf.size, 500)
                if (written >= 0) return
            }

            // Fallback: USB HID Class SET_REPORT Output Report via Control Endpoint 0
            val reportId = packet[0].toInt() and 0xFF
            val wValue = (HID_REPORT_TYPE_OUTPUT shl 8) or reportId
            val rc = conn.controlTransfer(
                HID_REQUEST_TYPE_OUT,
                HID_SET_REPORT,
                wValue,
                iface.id,
                packet,
                packet.size,
                500,
            )
            if (rc < 0) {
                throw MoondropUsbException("Error enviando reporte HID USB (código $rc).")
            }
        }

        private fun readHidReport(timeoutMs: Int): ByteArray? {
            val ep = epIn ?: return null
            val buf = ByteArray(maxOf(64, ep.maxPacketSize))
            val n = conn.bulkTransfer(ep, buf, buf.size, timeoutMs)
            return if (n > 0) buf.copyOf(n) else null
        }

        fun ktCommand(
            reg: Byte,
            cmd: Byte,
            value: ByteArray = ByteArray(4),
            ignoreDisconnect: Boolean = false,
        ): ByteArray {
            val pkt = MoondropProtocolCodec.ktPacket(reg, cmd, value)
            repeat(2) {
                drainInput()
                try {
                    sendHidReport(pkt)
                } catch (e: MoondropUsbException) {
                    if (ignoreDisconnect) return ByteArray(4)
                    throw e
                }

                repeat(8) {
                    val reply = readHidReport(450) ?: return@repeat
                    val extracted = MoondropProtocolCodec.ktExtractReplyValue(reply, reg, cmd)
                    if (extracted != null) return extracted
                }
                if (ignoreDisconnect) return ByteArray(4)
            }
            throw MoondropUsbException(
                String.format(
                    java.util.Locale.US,
                    "Sin respuesta del DSP para registro 0x%02X (cmd 0x%02X).",
                    reg.toInt() and 0xFF,
                    cmd.toInt() and 0xFF,
                )
            )
        }

        fun md8SendAndRead(packet: ByteArray): ByteArray {
            drainInput()
            sendHidReport(packet)
            repeat(8) {
                val reply = readHidReport(600) ?: return@repeat
                val off = if (reply.size > 1 && reply[0] == MoondropProtocolCodec.MD_REPORT_ID) 1 else 0
                if (reply.size >= off + 2 &&
                    reply[off] == MoondropProtocolCodec.MD_CMD_READ &&
                    reply[off + 1] == MoondropProtocolCodec.MD_CMD_UPDATE_EQ
                ) {
                    return reply
                }
            }
            throw MoondropUsbException("Sin respuesta del DSP Moondrop 8 bandas.")
        }
    }
}
