package com.speed17x.trivium.hardware

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
import com.speed17x.trivium.dsp.BiquadMath
import com.speed17x.trivium.dsp.FilterBand
import com.speed17x.trivium.dsp.FilterType
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.Locale
import kotlin.math.abs
import kotlin.math.roundToInt

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
        const val ACTION_USB_PERMISSION = "com.speed17x.trivium.USB_PERMISSION"
        private const val HID_REQUEST_TYPE_OUT = 0x21 // Host->Device | Class | Interface
        private const val HID_REQUEST_TYPE_IN = 0xA1  // Device->Host | Class | Interface
        private const val HID_GET_REPORT = 0x01
        private const val HID_SET_REPORT = 0x09
        private const val HID_REPORT_TYPE_OUTPUT = 0x02
        private const val HID_REPORT_TYPE_FEATURE = 0x03

        // USB CDC ACM control requests
        private const val CDC_SET_LINE_CODING = 0x20
        private const val CDC_SET_CONTROL_LINE_STATE = 0x22
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
            val hasHidOrCdc = (0 until d.interfaceCount).any {
                val cls = d.getInterface(it).interfaceClass
                cls == UsbConstants.USB_CLASS_HID || cls == UsbConstants.USB_CLASS_CDC_DATA
            }
            UsbDiagnosticEntry(
                deviceName = d.deviceName,
                productName = product.ifBlank { "USB ${d.vendorId.toString(16)}:${d.productId.toString(16)}" },
                vidPid = String.format(Locale.US, "%04X:%04X", d.vendorId, d.productId),
                interfaceCount = d.interfaceCount,
                hasHidInterface = hasHidOrCdc,
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

    fun readEq(info: MoondropDevice): Pair<Boolean, List<FilterBand>> {
        if (info.protocol == DspProtocol.JDS_ELEMENT) {
            return withCdcSession(info) { cdc ->
                val resp = cdc.exchangeJson(MoondropProtocolCodec.jdsBuildDescribePayload())
                val (_preamp, bands) = MoondropProtocolCodec.jdsParseDescribeResponse(resp)
                val enabled = bands.any { abs(it.gain) > 0.01 }
                enabled to bands
            }
        }
        return withHidSession(info) { session ->
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

                DspProtocol.MOONDROP_8B, DspProtocol.WALKPLAY -> {
                    val repId = info.reportId.toByte()
                    val bands = MutableList(info.maxBands) { i ->
                        val req = MoondropProtocolCodec.md8BuildReadPacket(i, repId)
                        val reply = session.md8SendAndRead(req)
                        MoondropProtocolCodec.md8DecodeReadReply(reply)
                            ?: FilterBand(FilterType.PEAKING, 1000.0, 0.0, 1.0)
                    }
                    true to bands
                }

                DspProtocol.FIIO -> {
                    val repId = info.reportId.toByte()
                    val presetReq = MoondropProtocolCodec.fiioBuildSimpleCmd(
                        repId,
                        isSet = false,
                        cmd = MoondropProtocolCodec.FIIO_CMD_PRESET_SWITCH,
                    )
                    val enabled = try {
                        val payload = session.fiioSendAndRead(
                            presetReq,
                            repId,
                            MoondropProtocolCodec.FIIO_CMD_PRESET_SWITCH,
                        )
                        (payload.getOrNull(6)?.toInt()?.and(0xFF) ?: info.userSlot) != info.bypassSlot
                    } catch (_: Exception) {
                        true
                    }
                    val bands = MutableList(info.maxBands) { FilterBand(FilterType.PEAKING, 1000.0, 0.0, 1.0) }
                    for (i in 0 until info.maxBands) {
                        val req = MoondropProtocolCodec.fiioBuildSimpleCmd(
                            repId,
                            isSet = false,
                            cmd = MoondropProtocolCodec.FIIO_CMD_FILTER_PARAMS,
                            arg = i,
                        )
                        try {
                            val payload = session.fiioSendAndRead(
                                req,
                                repId,
                                MoondropProtocolCodec.FIIO_CMD_FILTER_PARAMS,
                            )
                            val parsed = MoondropProtocolCodec.fiioDecodeFilterPayload(payload)
                            if (parsed != null && parsed.first in bands.indices) {
                                bands[parsed.first] = parsed.second
                            }
                        } catch (_: Exception) {
                        }
                    }
                    enabled to bands
                }

                DspProtocol.FOSI_DS3 -> {
                    val repId = info.reportId.toByte()
                    var enabled = true
                    try {
                        session.sendHidReport(
                            MoondropProtocolCodec.fosiBuildCmdPacket(
                                repId,
                                MoondropProtocolCodec.FOSI_CMD_GET_EQ_ENABLE,
                            ),
                        )
                        val ack = session.getFeatureReport(info.reportId, 64)
                        val off = if (ack.isNotEmpty() && ack[0] == MoondropProtocolCodec.FOSI_HEADER) 0 else 1
                        if (ack.size >= off + 4 &&
                            ack[off] == MoondropProtocolCodec.FOSI_HEADER &&
                            ack[off + 1] == MoondropProtocolCodec.FOSI_CMD_GET_EQ_ENABLE
                        ) {
                            enabled = ack[off + 2].toInt() == 1
                        }
                    } catch (_: Exception) {
                    }
                    try {
                        session.sendFeatureReport(
                            MoondropProtocolCodec.fosiBuildCmdPacket(
                                repId,
                                MoondropProtocolCodec.FOSI_CMD_SET_EQ_MODE,
                                info.userSlot,
                            ),
                        )
                        Thread.sleep(20)
                    } catch (_: Exception) {
                    }
                    val bands = MutableList(info.maxBands) { FilterBand(FilterType.PEAKING, 1000.0, 0.0, 1.0) }
                    for (i in 0 until info.maxBands) {
                        val req = ByteArray(64)
                        req[0] = repId
                        req[1] = MoondropProtocolCodec.FOSI_HEADER
                        req[2] = MoondropProtocolCodec.FOSI_CMD_GET_EQ_PARAMS
                        req[3] = info.userSlot.toByte()
                        req[4] = i.toByte()
                        try {
                            session.sendFeatureReport(req)
                            val resp = session.getFeatureReport(info.reportId, 64)
                            val parsed = MoondropProtocolCodec.fosiDecodeBandPacket(resp)
                            if (parsed != null && parsed.first in bands.indices) {
                                bands[parsed.first] = parsed.second
                            }
                        } catch (_: Exception) {
                        }
                        Thread.sleep(15)
                    }
                    enabled to bands
                }

                DspProtocol.TOPPING -> {
                    session.sendHidReport(
                        MoondropProtocolCodec.toppingBuildFrame(
                            MoondropProtocolCodec.TOPPING_PTYPE_WRITE_NACK,
                            MoondropProtocolCodec.TOPPING_CMD_CONNECT_STATE,
                            1,
                        ),
                    )
                    Thread.sleep(80)
                    session.sendHidReport(
                        MoondropProtocolCodec.toppingBuildFrame(
                            MoondropProtocolCodec.TOPPING_PTYPE_WRITE_NACK,
                            MoondropProtocolCodec.TOPPING_CMD_AGREEMENT_CONFIG,
                            1,
                        ),
                    )
                    Thread.sleep(80)
                    session.drainInput()
                    session.sendHidReport(
                        MoondropProtocolCodec.toppingBuildFrame(
                            MoondropProtocolCodec.TOPPING_PTYPE_READ_NACK,
                            MoondropProtocolCodec.TOPPING_CMD_UPLOAD,
                            0,
                        ),
                    )
                    val frames = HashMap<Int, Long>()
                    var totalLen: Int? = null
                    val deadline = System.currentTimeMillis() + 2500L
                    while (System.currentTimeMillis() < deadline) {
                        val raw = session.readHidReport(250) ?: continue
                        val f = MoondropProtocolCodec.toppingParseFrame(raw) ?: continue
                        if (f.cmd == MoondropProtocolCodec.TOPPING_CMD_UPLOAD && f.totalFrameLen >= 70) {
                            if (totalLen == null) totalLen = f.totalFrameLen
                            frames[f.curFrame] = f.dataU32
                            val tl = totalLen
                            if (tl != null && frames.size >= tl && (0 until tl).all { it in frames }) {
                                break
                            }
                        }
                    }
                    val enabled = (frames[4] ?: 1L) != 0L
                    val bands = List(info.maxBands) { i ->
                        val base = 8 + i * 3
                        val p = frames[base]
                        val freq = frames[base + 1]
                        val q = frames[base + 2]
                        if (p != null && freq != null && q != null) {
                            MoondropProtocolCodec.toppingDecodePackedBand(p, freq, q).second
                        } else {
                            FilterBand(FilterType.PEAKING, 1000.0, 0.0, 0.707)
                        }
                    }
                    enabled to bands
                }

                DspProtocol.QUDELIX_5K -> {
                    val repId = info.reportId.toByte()
                    session.drainInput()
                    try {
                        session.sendHidReport(
                            MoondropProtocolCodec.qudelixBuildPacket(
                                repId,
                                MoondropProtocolCodec.QUDELIX_CMD_REQ_INIT_DATA,
                            ),
                        )
                        Thread.sleep(20)
                        session.sendHidReport(
                            MoondropProtocolCodec.qudelixBuildPacket(
                                repId,
                                MoondropProtocolCodec.QUDELIX_CMD_REQ_DEV_CONFIG,
                                byteArrayOf(0x3C),
                            ),
                        )
                        Thread.sleep(15)
                        session.sendHidReport(
                            MoondropProtocolCodec.qudelixBuildPacket(
                                repId,
                                MoondropProtocolCodec.QUDELIX_CMD_REQ_DEV_CONFIG,
                                byteArrayOf(0xC0.toByte()),
                            ),
                        )
                        Thread.sleep(15)
                        session.sendHidReport(
                            MoondropProtocolCodec.qudelixBuildPacket(
                                repId,
                                MoondropProtocolCodec.QUDELIX_CMD_REQ_DEV_STATUS,
                                byteArrayOf(0x04),
                            ),
                        )
                        Thread.sleep(15)
                        session.sendHidReport(
                            MoondropProtocolCodec.qudelixBuildPacket(
                                repId,
                                MoondropProtocolCodec.QUDELIX_CMD_REQ_EQ_PRESET,
                                byteArrayOf(0x03),
                            ),
                        )
                    } catch (_: Exception) {
                    }

                    val presetBuf = ByteArray(128)
                    var gotPreset = false
                    val deadline = System.currentTimeMillis() + 900L
                    while (System.currentTimeMillis() < deadline) {
                        val raw = session.readHidReport(120) ?: break
                        val off = if (raw.size >= 4 && (raw[0].toInt() == 9 || raw[0].toInt() == 1)) 1 else 0
                        if (raw.size >= off + 10) {
                            val len = raw[off].toInt() and 0xFF
                            val cmd = ((raw[off + 1].toInt() and 0xFF) shl 8) or (raw[off + 2].toInt() and 0xFF)
                            if (cmd == MoondropProtocolCodec.QUDELIX_CMD_RSP_EQ_PRESET) {
                                val end = minOf(off + 1 + len, raw.size)
                                val d = raw.copyOfRange(off + 3, end)
                                if (d.size >= 7 && d[0].toInt() == 0) {
                                    val totalPkts = (d[1].toInt() and 0xFF) ushr 4
                                    val pktIdx = d[1].toInt() and 0x0F
                                    val chunkOff = ((d[4].toInt() and 0xFF) shl 8) or (d[5].toInt() and 0xFF)
                                    val chunk = d.copyOfRange(6, d.size)
                                    if (chunkOff + chunk.size <= presetBuf.size) {
                                        System.arraycopy(chunk, 0, presetBuf, chunkOff, chunk.size)
                                    }
                                    if (pktIdx == totalPkts) {
                                        gotPreset = true
                                        break
                                    }
                                }
                            }
                        }
                    }

                    if (gotPreset) {
                        try {
                            val (_preamp, bands) = MoondropProtocolCodec.qudelixParseUserEqPreset(presetBuf)
                            return@withHidSession true to bands
                        } catch (_: Exception) {
                        }
                    }
                    val defFreqs = doubleArrayOf(32.0, 64.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0)
                    true to defFreqs.map { FilterBand(FilterType.PEAKING, it, 0.0, 1.0) }
                }

                DspProtocol.CONEXANT_9B -> {
                    val bands = MoondropProtocolCodec.CONEXANT_FIXED_FREQS.map { f ->
                        FilterBand(FilterType.PEAKING, f, 0.0, 1.0)
                    }
                    true to bands
                }

                DspProtocol.JDS_ELEMENT -> throw MoondropUsbException("JDS Labs Element IV usa transporte USB-CDC.")
                DspProtocol.FIIO_BLE, DspProtocol.AIROHA_BLE -> throw MoondropUsbException("Dispositivo Bluetooth BLE GATT, no USB HID.")
                DspProtocol.SYSTEM_DYNAMICS -> throw MoondropUsbException("Este dispositivo usa EQ del sistema (no USB HID).")
            }
        }
    }

    fun writeEq(
        info: MoondropDevice,
        bands: List<FilterBand>,
        commit: Boolean = false,
        enableEq: Boolean = true,
    ) {
        val fitted = bands.take(info.maxBands).map {
            it.clamped(maxGainDb = info.maxGainDb, allowShelves = info.supportsShelves)
        }.toMutableList()
        while (fitted.size < info.maxBands) {
            fitted += FilterBand(FilterType.PEAKING, 1000.0, 0.0, 1.0)
        }

        if (info.protocol == DspProtocol.JDS_ELEMENT) {
            withCdcSession(info) { cdc ->
                val aligned = MoondropProtocolCodec.jdsAlign12Bands(fitted)
                val preamp = if (enableEq) BiquadMath.computeSafePreampDb(aligned) else 0.0
                val req = MoondropProtocolCodec.jdsBuildUpdatePayload(aligned, preamp, enableEq)
                val resp = cdc.exchangeJson(req)
                val root = Json.parseToJsonElement(resp.trim('\u0000').trim()).jsonObject
                if (root["Status"]?.jsonPrimitive?.booleanOrNull != true) {
                    throw MoondropUsbException("JDS Labs Element IV rechazó la actualización EQ.")
                }
            }
            return
        }

        withHidSession(info) { session ->
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
                    val repId = info.reportId.toByte()
                    for (i in fitted.indices) {
                        val b = if (enableEq) fitted[i] else FilterBand(FilterType.PEAKING, fitted[i].frequency, 0.0, 1.0)
                        session.sendHidReport(MoondropProtocolCodec.md8BuildWritePacket(i, b, info.userSlot, repId))
                        session.sendHidReport(MoondropProtocolCodec.md8BuildApplyPacket(i, repId))
                        Thread.sleep(15)
                    }
                    if (commit) {
                        session.sendHidReport(MoondropProtocolCodec.md8BuildSaveFlashPacket(repId))
                    }
                }

                DspProtocol.WALKPLAY -> {
                    val repId = info.reportId.toByte()
                    for (i in fitted.indices) {
                        val b = if (enableEq) fitted[i] else FilterBand(FilterType.PEAKING, fitted[i].frequency, 0.0, 1.0)
                        session.sendHidReport(MoondropProtocolCodec.md8BuildWritePacket(i, b, info.userSlot, repId))
                        Thread.sleep(18)
                    }
                    Thread.sleep(60)
                    val preamp = if (enableEq) BiquadMath.computeSafePreampDb(fitted) else 0.0
                    session.sendHidReport(MoondropProtocolCodec.wpBuildGlobalGainPacket(repId, preamp))
                    Thread.sleep(25)
                    for (pkt in MoondropProtocolCodec.wpBuildCommitPackets(repId, commit)) {
                        session.sendHidReport(pkt)
                        Thread.sleep(20)
                    }
                }

                DspProtocol.FIIO -> {
                    val repId = info.reportId.toByte()
                    val preamp = if (enableEq) BiquadMath.computeSafePreampDb(fitted) else 0.0
                    session.sendHidReport(MoondropProtocolCodec.fiioBuildGlobalGainPacket(repId, preamp))
                    session.sendHidReport(
                        MoondropProtocolCodec.fiioBuildSimpleCmd(
                            repId,
                            isSet = true,
                            cmd = MoondropProtocolCodec.FIIO_CMD_FILTER_COUNT,
                            arg = fitted.size,
                        ),
                    )
                    Thread.sleep(60)
                    for (i in fitted.indices) {
                        val b = if (enableEq) fitted[i] else FilterBand(FilterType.PEAKING, fitted[i].frequency, 0.0, 1.0)
                        session.sendHidReport(MoondropProtocolCodec.fiioBuildSetFilterPacket(repId, i, b))
                        Thread.sleep(10)
                    }
                    Thread.sleep(60)
                    val targetSlot = if (enableEq) info.userSlot else info.bypassSlot
                    session.sendHidReport(
                        MoondropProtocolCodec.fiioBuildSimpleCmd(
                            repId,
                            isSet = true,
                            cmd = MoondropProtocolCodec.FIIO_CMD_PRESET_SWITCH,
                            arg = targetSlot,
                        ),
                    )
                    if (commit) {
                        val saveCmd = if (info.fiioSaveV2) MoondropProtocolCodec.FIIO_CMD_SAVE_V2 else MoondropProtocolCodec.FIIO_CMD_SAVE_TO_DEVICE
                        session.sendHidReport(
                            MoondropProtocolCodec.fiioBuildSimpleCmd(repId, isSet = true, cmd = saveCmd, arg = info.userSlot),
                        )
                    }
                }

                DspProtocol.FOSI_DS3 -> {
                    val repId = info.reportId.toByte()
                    session.sendFeatureReport(
                        MoondropProtocolCodec.fosiBuildCmdPacket(repId, MoondropProtocolCodec.FOSI_CMD_GET_EQ_MODE_COUNT, 0),
                    )
                    Thread.sleep(50)
                    session.sendFeatureReport(
                        MoondropProtocolCodec.fosiBuildCmdPacket(repId, MoondropProtocolCodec.FOSI_CMD_SET_EQ_MODE, info.userSlot),
                    )
                    Thread.sleep(30)
                    for (i in fitted.indices) {
                        val b = if (enableEq) fitted[i] else FilterBand(FilterType.PEAKING, fitted[i].frequency, 0.0, 1.0)
                        session.sendFeatureReport(MoondropProtocolCodec.fosiEncodeBandPacket(repId, info.userSlot, i, b))
                        Thread.sleep(20)
                        val commitBand = ByteArray(64)
                        commitBand[0] = repId
                        commitBand[1] = MoondropProtocolCodec.FOSI_HEADER
                        commitBand[2] = MoondropProtocolCodec.FOSI_CMD_GET_EQ_PARAMS
                        commitBand[3] = info.userSlot.toByte()
                        commitBand[4] = i.toByte()
                        session.sendHidReport(commitBand)
                        Thread.sleep(20)
                    }
                    if (commit) {
                        session.sendFeatureReport(
                            MoondropProtocolCodec.fosiBuildCmdPacket(repId, MoondropProtocolCodec.FOSI_CMD_SET_AND_SAVE_EQ_MODE, info.userSlot),
                        )
                        Thread.sleep(50)
                    }
                    session.sendHidReport(
                        MoondropProtocolCodec.fosiBuildCmdPacket(
                            repId,
                            MoondropProtocolCodec.FOSI_CMD_SET_EQ_ENABLE,
                            if (enableEq) 1 else 0,
                        ),
                    )
                    Thread.sleep(30)
                    if (enableEq) {
                        session.sendFeatureReport(
                            MoondropProtocolCodec.fosiBuildCmdPacket(repId, MoondropProtocolCodec.FOSI_CMD_SET_EQ_MODE, info.userSlot),
                        )
                    }
                }

                DspProtocol.TOPPING -> {
                    session.sendHidReport(
                        MoondropProtocolCodec.toppingBuildFrame(
                            MoondropProtocolCodec.TOPPING_PTYPE_WRITE_NACK,
                            MoondropProtocolCodec.TOPPING_CMD_CONNECT_STATE,
                            1,
                        ),
                    )
                    Thread.sleep(80)
                    session.sendHidReport(
                        MoondropProtocolCodec.toppingBuildFrame(
                            MoondropProtocolCodec.TOPPING_PTYPE_WRITE_NACK,
                            MoondropProtocolCodec.TOPPING_CMD_AGREEMENT_CONFIG,
                            1,
                        ),
                    )
                    Thread.sleep(80)
                    session.sendHidReport(
                        MoondropProtocolCodec.toppingBuildFrame(
                            MoondropProtocolCodec.TOPPING_PTYPE_WRITE_NACK,
                            MoondropProtocolCodec.TOPPING_CMD_MCU_EQ_ENABLE,
                            if (enableEq) 1 else 0,
                        ),
                    )
                    Thread.sleep(30)

                    for (i in 0 until minOf(fitted.size, 11)) {
                        val b = if (enableEq) fitted[i] else FilterBand(FilterType.PEAKING, fitted[i].frequency, 0.0, 0.707)
                        val baseCmd = 0x9000 or ((i + 1) shl 8)
                        val typeCode = MoondropProtocolCodec.toppingTypeToCode(b.type)
                        val freq = b.frequency.roundToInt().coerceIn(20, 20000)
                        val gainByte = MoondropProtocolCodec.toppingEncodeGainByte(b.gain)
                        val qRaw = (b.q * 10000.0).roundToInt()
                        val enVal = if (abs(b.gain) > 1e-4) 1 else 0

                        val pairs = listOf(
                            1 to typeCode,
                            2 to freq,
                            3 to gainByte,
                            4 to qRaw,
                            5 to enVal,
                            6 to typeCode,
                            7 to freq,
                            8 to gainByte,
                            9 to qRaw,
                            10 to enVal,
                        )
                        for ((sub, value) in pairs) {
                            session.sendHidReport(
                                MoondropProtocolCodec.toppingBuildFrame(
                                    MoondropProtocolCodec.TOPPING_PTYPE_WRITE_NACK,
                                    baseCmd or sub,
                                    value,
                                ),
                            )
                            Thread.sleep(4)
                        }
                        session.sendHidReport(
                            MoondropProtocolCodec.toppingBuildFrame(
                                MoondropProtocolCodec.TOPPING_PTYPE_WRITE_NACK,
                                MoondropProtocolCodec.TOPPING_CMD_CONFIG_SWITCH,
                                0,
                            ),
                        )
                        if (i % 3 == 2) {
                            session.sendHidReport(
                                MoondropProtocolCodec.toppingBuildFrame(
                                    MoondropProtocolCodec.TOPPING_PTYPE_WRITE_NACK,
                                    MoondropProtocolCodec.TOPPING_CMD_HEARTBEAT,
                                    1,
                                ),
                            )
                        }
                        Thread.sleep(20)
                    }
                    if (commit) {
                        session.sendHidReport(
                            MoondropProtocolCodec.toppingBuildFrame(
                                MoondropProtocolCodec.TOPPING_PTYPE_WRITE_NACK,
                                MoondropProtocolCodec.TOPPING_CMD_EQ_PREVIEW,
                                1,
                            ),
                        )
                    }
                }

                DspProtocol.QUDELIX_5K -> {
                    val repId = info.reportId.toByte()
                    session.sendHidReport(
                        MoondropProtocolCodec.qudelixBuildPacket(
                            repId,
                            MoondropProtocolCodec.QUDELIX_CMD_SET_EQ_ENABLE,
                            byteArrayOf(0x00, if (enableEq) 1 else 0),
                        ),
                    )
                    Thread.sleep(15)
                    session.sendHidReport(
                        MoondropProtocolCodec.qudelixBuildPacket(
                            repId,
                            MoondropProtocolCodec.QUDELIX_CMD_SET_EQ_TYPE,
                            byteArrayOf(0x00, 0x01),
                        ),
                    )
                    Thread.sleep(15)
                    val preamp = if (enableEq) (BiquadMath.computeSafePreampDb(fitted) * 10.0).roundToInt() else 0
                    session.sendHidReport(
                        MoondropProtocolCodec.qudelixBuildPacket(
                            repId,
                            MoondropProtocolCodec.QUDELIX_CMD_SET_EQ_PREGAIN,
                            byteArrayOf(0x00, 0x03, 0x00, ((preamp ushr 8) and 0xFF).toByte(), (preamp and 0xFF).toByte()),
                        ),
                    )
                    Thread.sleep(15)
                    for (i in 0 until minOf(fitted.size, 10)) {
                        val b = if (enableEq) fitted[i] else FilterBand(FilterType.PEAKING, fitted[i].frequency, 0.0, 1.0)
                        session.sendHidReport(MoondropProtocolCodec.qudelixBuildBandPacket(repId, i, b))
                        Thread.sleep(15)
                    }
                    if (commit) {
                        session.sendHidReport(
                            MoondropProtocolCodec.qudelixBuildPacket(
                                repId,
                                MoondropProtocolCodec.QUDELIX_CMD_SAVE_EQ_PRESET,
                                byteArrayOf(info.userSlot.toByte()),
                            ),
                        )
                    }
                }

                DspProtocol.CONEXANT_9B -> {
                    for (i in 0 until minOf(fitted.size, MoondropProtocolCodec.CONEXANT_FIXED_FREQS.size)) {
                        val b = if (enableEq) fitted[i] else FilterBand(FilterType.PEAKING, fitted[i].frequency, 0.0, 1.0)
                        for (pkt in MoondropProtocolCodec.conexantBuildBandPackets(i + 1, b)) {
                            session.sendHidReport(pkt)
                            Thread.sleep(15)
                        }
                    }
                    session.sendHidReport(MoondropProtocolCodec.conexantModePacket(0))
                }

                DspProtocol.JDS_ELEMENT -> Unit
                DspProtocol.FIIO_BLE, DspProtocol.AIROHA_BLE -> throw MoondropUsbException("Dispositivo Bluetooth BLE GATT, no USB HID.")
                DspProtocol.SYSTEM_DYNAMICS -> throw MoondropUsbException("Este dispositivo usa EQ del sistema (no USB HID).")
            }
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

    private fun <T> withCdcSession(info: MoondropDevice, block: (CdcSession) -> T): T = synchronized(usbSessionLock) {
        val mgr = usbManager ?: throw MoondropUsbException("Servicio USB no disponible en este dispositivo.")
        val usbDev = findUsbDevice(info)
            ?: throw MoondropUsbException("${info.displayName} no está conectado por USB-C.")
        if (!mgr.hasPermission(usbDev)) {
            throw MoondropUsbException("Sin permiso USB para ${info.displayName}. Pulsa 'Autorizar USB'.")
        }

        val dataIface = (0 until usbDev.interfaceCount)
            .map { usbDev.getInterface(it) }
            .firstOrNull { iface ->
                iface.interfaceClass == UsbConstants.USB_CLASS_CDC_DATA &&
                    (0 until iface.endpointCount).any { iface.getEndpoint(it).direction == UsbConstants.USB_DIR_OUT } &&
                    (0 until iface.endpointCount).any { iface.getEndpoint(it).direction == UsbConstants.USB_DIR_IN }
            } ?: throw MoondropUsbException("No se encontró interfaz CDC Serial en ${info.displayName}.")

        val commIface = (0 until usbDev.interfaceCount)
            .map { usbDev.getInterface(it) }
            .firstOrNull { it.interfaceClass == UsbConstants.USB_CLASS_COMM }

        val conn = mgr.openDevice(usbDev)
            ?: throw MoondropUsbException("No se pudo abrir la conexión USB-CDC con ${info.displayName}.")

        commIface?.let { conn.claimInterface(it, true) }
        if (!conn.claimInterface(dataIface, true)) {
            commIface?.let { conn.releaseInterface(it) }
            conn.close()
            throw MoondropUsbException("No se pudo reclamar la interfaz CDC de ${info.displayName}.")
        }

        return try {
            val session = CdcSession(conn, commIface, dataIface)
            session.configureLine115200()
            block(session)
        } finally {
            try {
                conn.releaseInterface(dataIface)
            } catch (_: Exception) {
            }
            commIface?.let {
                try {
                    conn.releaseInterface(it)
                } catch (_: Exception) {
                }
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

    private class CdcSession(
        private val conn: UsbDeviceConnection,
        private val commIface: UsbInterface?,
        dataIface: UsbInterface,
    ) {
        private val epIn: UsbEndpoint
        private val epOut: UsbEndpoint

        init {
            var inEp: UsbEndpoint? = null
            var outEp: UsbEndpoint? = null
            for (i in 0 until dataIface.endpointCount) {
                val ep = dataIface.getEndpoint(i)
                if (ep.direction == UsbConstants.USB_DIR_IN && inEp == null) inEp = ep
                if (ep.direction == UsbConstants.USB_DIR_OUT && outEp == null) outEp = ep
            }
            epIn = inEp ?: throw MoondropUsbException("Sin endpoint Bulk IN en CDC.")
            epOut = outEp ?: throw MoondropUsbException("Sin endpoint Bulk OUT en CDC.")
        }

        fun configureLine115200() {
            val ifId = commIface?.id ?: 0
            // 115200 bps, 1 stop bit, no parity, 8 data bits
            val coding = ByteBuffer.allocate(7).order(ByteOrder.LITTLE_ENDIAN)
                .putInt(115200)
                .put(0.toByte())
                .put(0.toByte())
                .put(8.toByte())
                .array()
            conn.controlTransfer(HID_REQUEST_TYPE_OUT, CDC_SET_LINE_CODING, 0, ifId, coding, coding.size, 300)
            conn.controlTransfer(HID_REQUEST_TYPE_OUT, CDC_SET_CONTROL_LINE_STATE, 0x03, ifId, null, 0, 300)
        }

        fun exchangeJson(payload: ByteArray): String {
            // Drain stale RX
            val tmp = ByteArray(maxOf(64, epIn.maxPacketSize))
            repeat(4) {
                if (conn.bulkTransfer(epIn, tmp, tmp.size, 10) <= 0) return@repeat
            }

            val written = conn.bulkTransfer(epOut, payload, payload.size, 1000)
            if (written < 0) {
                throw MoondropUsbException("Error escribiendo JSON por USB-CDC ($written).")
            }

            val acc = ArrayList<Byte>(4096)
            val deadline = System.currentTimeMillis() + 2000L
            while (System.currentTimeMillis() < deadline) {
                val n = conn.bulkTransfer(epIn, tmp, tmp.size, 250)
                if (n > 0) {
                    for (i in 0 until n) {
                        val b = tmp[i]
                        if (b.toInt() == 0) {
                            return acc.toByteArray().toString(Charsets.UTF_8)
                        }
                        acc.add(b)
                    }
                }
            }
            if (acc.isNotEmpty()) return acc.toByteArray().toString(Charsets.UTF_8)
            throw MoondropUsbException("Timeout esperando respuesta JSON de JDS Labs Element IV.")
        }
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

        fun drainInput() {
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

        fun sendFeatureReport(packet: ByteArray) {
            val reportId = packet[0].toInt() and 0xFF
            val wValue = (HID_REPORT_TYPE_FEATURE shl 8) or reportId
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
                throw MoondropUsbException("Error enviando feature report HID USB (código $rc).")
            }
        }

        fun getFeatureReport(reportId: Int, length: Int = 64): ByteArray {
            val buf = ByteArray(length)
            buf[0] = (reportId and 0xFF).toByte()
            val wValue = (HID_REPORT_TYPE_FEATURE shl 8) or (reportId and 0xFF)
            val rc = conn.controlTransfer(
                HID_REQUEST_TYPE_IN,
                HID_GET_REPORT,
                wValue,
                iface.id,
                buf,
                buf.size,
                500,
            )
            if (rc < 0) {
                throw MoondropUsbException("Error leyendo feature report HID USB (código $rc).")
            }
            return buf.copyOf(rc)
        }

        fun readHidReport(timeoutMs: Int): ByteArray? {
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
                    Locale.US,
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

        fun fiioSendAndRead(packet: ByteArray, reportId: Byte, expectCmd: Byte): ByteArray {
            drainInput()
            sendHidReport(packet)
            repeat(8) {
                val reply = readHidReport(600) ?: return@repeat
                val off = if (reply.size > 2 && reply[0] == reportId && reply[1] == MoondropProtocolCodec.FIIO_GET_H1) 1 else 0
                if (reply.size >= off + 7 &&
                    reply[off] == MoondropProtocolCodec.FIIO_GET_H1 &&
                    reply[off + 1] == MoondropProtocolCodec.FIIO_GET_H2 &&
                    reply[off + 4] == expectCmd
                ) {
                    return reply.copyOfRange(off, reply.size)
                }
            }
            throw MoondropUsbException("Sin respuesta del DSP FiiO.")
        }
    }
}
