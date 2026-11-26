package com.hector.moondropdsp.hardware

import com.hector.moondropdsp.dsp.BiquadMath
import com.hector.moondropdsp.dsp.FilterBand
import com.hector.moondropdsp.dsp.FilterType
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.doubleOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.math.roundToInt

/**
 * Pure JVM protocol codecs achieving 100% parity with Rust `autoq-core`:
 * 1. KTMicro 5-band (`0x31B2`)
 * 2. Moondrop 8-band (`0x4B`)
 * 3. WalkPlay (5-10B, custom slot 101 + global gain + commit)
 * 4. FiiO / JadeAudio / SNOWSKY (5-10B, big-endian `AA 0A` / `BB 0B`)
 * 5. Fosi Audio DS3 (8B `0x77` Feature/Output float32 LE)
 * 6. Topping DX1 II / E50 II (11B, 16-byte `0x22 0x33 ... 0x66 0x77 0x00` + CRC-16/MODBUS)
 * 7. Qudelix-5K (10B, `SetEqBandParam 0x070F` + V3 88-byte LSB-first bitstream)
 * 8. JDS Labs Element IV (12B USB-CDC JSON `Describe` / `Update`)
 * 9. Moondrop Bluetooth Classic SPP (RFCOMM)
 */
object MoondropProtocolCodec {

    // ---- KTMicro 5-band USB HID constants ----
    const val KT_REPORT_ID: Byte = 0x4B
    const val KT_READ: Byte = 0x52
    const val KT_WRITE: Byte = 0x57
    const val KT_COMMIT: Byte = 0x53
    const val KT_REG_SLOT: Byte = 0x24
    const val KT_REG_BAND0: Byte = 0x26
    const val KT_SLOT_ON: Byte = 0x03
    const val KT_SLOT_OFF: Byte = 0x02

    // ---- Moondrop 8-band & WalkPlay USB HID constants ----
    const val MD_REPORT_ID: Byte = 0x4B
    const val MD_CMD_WRITE: Byte = 0x01
    const val MD_CMD_READ: Byte = 0x80.toByte()
    const val MD_CMD_UPDATE_EQ: Byte = 0x09
    const val MD_CMD_APPLY_COEFF: Byte = 0x0A
    const val MD_CMD_SAVE_FLASH: Byte = 0x01
    const val WP_CMD_GLOBAL_GAIN: Byte = 0x03

    // ---- FiiO / JadeAudio / SNOWSKY HID constants ----
    const val FIIO_SET_H1: Byte = 0xAA.toByte()
    const val FIIO_SET_H2: Byte = 0x0A
    const val FIIO_GET_H1: Byte = 0xBB.toByte()
    const val FIIO_GET_H2: Byte = 0x0B
    const val FIIO_END: Byte = 0xEE.toByte()
    const val FIIO_CMD_FILTER_PARAMS: Byte = 0x15
    const val FIIO_CMD_PRESET_SWITCH: Byte = 0x16
    const val FIIO_CMD_GLOBAL_GAIN: Byte = 0x17
    const val FIIO_CMD_FILTER_COUNT: Byte = 0x18
    const val FIIO_CMD_SAVE_TO_DEVICE: Byte = 0x19
    const val FIIO_CMD_SAVE_V2: Byte = 0x21

    // ---- Fosi Audio DS3 constants ----
    const val FOSI_HEADER: Byte = 0x77
    const val FOSI_CMD_SET_EQ_MODE: Byte = 0x8A.toByte()
    const val FOSI_CMD_SET_EQ_PARAMS: Byte = 0x8D.toByte()
    const val FOSI_CMD_GET_EQ_PARAMS: Byte = 0x8E.toByte()
    const val FOSI_CMD_GET_EQ_MODE_COUNT: Byte = 0x91.toByte()
    const val FOSI_CMD_SET_AND_SAVE_EQ_MODE: Byte = 0x92.toByte()
    const val FOSI_CMD_SET_EQ_ENABLE: Byte = 0x9D.toByte()
    const val FOSI_CMD_GET_EQ_ENABLE: Byte = 0x9E.toByte()

    // ---- Topping PEQ constants ----
    const val TOPPING_PTYPE_READ_NACK: Byte = 16
    const val TOPPING_PTYPE_WRITE_NACK: Byte = 32
    const val TOPPING_CMD_CONNECT_STATE = 0x1101
    const val TOPPING_CMD_UPLOAD = 0x1106
    const val TOPPING_CMD_HEARTBEAT = 0x111A
    const val TOPPING_CMD_EQ_PREVIEW = 0x111B
    const val TOPPING_CMD_AGREEMENT_CONFIG = 0x1120
    const val TOPPING_CMD_MCU_EQ_ENABLE = 0x1204
    const val TOPPING_CMD_CONFIG_SWITCH = 0x9E01

    // ---- Qudelix-5K constants ----
    const val QUDELIX_CMD_REQ_INIT_DATA = 0x0100
    const val QUDELIX_CMD_REQ_DEV_STATUS = 0x0110
    const val QUDELIX_CMD_REQ_DEV_CONFIG = 0x0120
    const val QUDELIX_CMD_REQ_EQ_PRESET = 0x0123
    const val QUDELIX_CMD_RSP_EQ_PRESET = 0x0128
    const val QUDELIX_CMD_SET_EQ_ENABLE = 0x0700
    const val QUDELIX_CMD_SET_EQ_TYPE = 0x0701
    const val QUDELIX_CMD_SET_EQ_PREGAIN = 0x0703
    const val QUDELIX_CMD_SAVE_EQ_PRESET = 0x0708
    const val QUDELIX_CMD_SET_EQ_BAND_PARAM = 0x070F

    // ---- JDS Labs Element IV constants ----
    val JDS_12_BAND_ORDER = listOf(
        "Lowshelf 1",
        "Lowshelf 2",
        "Peaking 1",
        "Peaking 2",
        "Peaking 3",
        "Peaking 4",
        "Peaking 5",
        "Peaking 6",
        "Peaking 7",
        "Peaking 8",
        "Highshelf 1",
        "Highshelf 2",
    )

    private fun ktTypeToCode(type: FilterType): Byte = when (type) {
        FilterType.PEAKING -> 0
        FilterType.LOW_SHELF -> 3
        FilterType.HIGH_SHELF -> 4
    }

    private fun ktCodeToType(code: Int): FilterType = when (code) {
        3 -> FilterType.LOW_SHELF
        4 -> FilterType.HIGH_SHELF
        else -> FilterType.PEAKING
    }

    private fun mdTypeToCode(type: FilterType): Byte = when (type) {
        FilterType.LOW_SHELF -> 1
        FilterType.PEAKING -> 2
        FilterType.HIGH_SHELF -> 3
    }

    private fun mdCodeToType(code: Int): FilterType = when (code) {
        1 -> FilterType.LOW_SHELF
        3 -> FilterType.HIGH_SHELF
        else -> FilterType.PEAKING
    }

    private fun fiioTypeToCode(type: FilterType): Byte = when (type) {
        FilterType.PEAKING -> 0
        FilterType.LOW_SHELF -> 1
        FilterType.HIGH_SHELF -> 2
    }

    private fun fiioCodeToType(code: Int): FilterType = when (code) {
        1 -> FilterType.LOW_SHELF
        2 -> FilterType.HIGH_SHELF
        else -> FilterType.PEAKING
    }

    private fun fosiTypeToCode(type: FilterType): Byte = when (type) {
        FilterType.PEAKING -> 2
        FilterType.LOW_SHELF -> 9
        FilterType.HIGH_SHELF -> 10
    }

    private fun fosiCodeToType(code: Int): FilterType = when (code) {
        9 -> FilterType.LOW_SHELF
        10 -> FilterType.HIGH_SHELF
        else -> FilterType.PEAKING
    }

    fun toppingTypeToCode(type: FilterType): Int = when (type) {
        FilterType.PEAKING -> 1
        FilterType.LOW_SHELF -> 2
        FilterType.HIGH_SHELF -> 3
    }

    private fun toppingCodeToType(code: Int): FilterType = when (code) {
        2 -> FilterType.LOW_SHELF
        3 -> FilterType.HIGH_SHELF
        else -> FilterType.PEAKING
    }

    private fun qudelixTypeToCode(type: FilterType): Byte = when (type) {
        FilterType.LOW_SHELF -> 3
        FilterType.HIGH_SHELF -> 4
        FilterType.PEAKING -> 5
    }

    private fun qudelixCodeToType(code: Int): FilterType = when (code) {
        3 -> FilterType.LOW_SHELF
        4 -> FilterType.HIGH_SHELF
        else -> FilterType.PEAKING
    }

    // ---- 1. KTMicro ----

    fun ktPacket(reg: Byte, cmd: Byte, value: ByteArray = ByteArray(4)): ByteArray {
        val pkt = ByteArray(11)
        pkt[0] = KT_REPORT_ID
        pkt[1] = reg
        pkt[5] = cmd
        val len = minOf(4, value.size)
        System.arraycopy(value, 0, pkt, 7, len)
        return pkt
    }

    fun ktEncodeBand(band: FilterBand): Pair<ByteArray, ByteArray> {
        val b = band.clamped()
        val gf = ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN)
        gf.putShort((b.gain * 10.0).roundToInt().toShort())
        gf.putShort(b.frequency.roundToInt().toShort())

        val qt = ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN)
        qt.putShort((b.q * 1000.0).roundToInt().toShort())
        qt.put(ktTypeToCode(b.type))
        qt.put(0)

        return gf.array() to qt.array()
    }

    fun ktDecodeBand(gainFreq: ByteArray, qType: ByteArray): FilterBand {
        val gf = ByteBuffer.wrap(gainFreq).order(ByteOrder.LITTLE_ENDIAN)
        val gain = gf.short.toInt() / 10.0
        val freq = (gf.short.toInt() and 0xFFFF).toDouble()

        val qt = ByteBuffer.wrap(qType).order(ByteOrder.LITTLE_ENDIAN)
        val q = (qt.short.toInt() and 0xFFFF) / 1000.0
        val ftype = ktCodeToType(qType[2].toInt() and 0xFF)
        return FilterBand(type = ftype, frequency = freq, gain = gain, q = q).clamped()
    }

    fun ktExtractReplyValue(reply: ByteArray, reg: Byte, cmd: Byte): ByteArray? {
        if (reply.size >= 11 && reply[0] == KT_REPORT_ID && reply[1] == reg && reply[5] == cmd) {
            return reply.copyOfRange(7, 11)
        }
        if (reply.size >= 10 && reply[0] == reg && reply[4] == cmd) {
            return reply.copyOfRange(6, 10)
        }
        return null
    }

    // ---- 2. Moondrop 8-band & WalkPlay ----

    fun md8BuildReadPacket(index: Int, reportId: Byte = MD_REPORT_ID): ByteArray {
        val pkt = ByteArray(64)
        pkt[0] = reportId
        pkt[1] = MD_CMD_READ
        pkt[2] = MD_CMD_UPDATE_EQ
        pkt[3] = 0x18
        pkt[4] = 0x00
        pkt[5] = index.toByte()
        pkt[6] = 0x00
        return pkt
    }

    fun md8BuildWritePacket(
        index: Int,
        band: FilterBand,
        slot: Int = 7,
        reportId: Byte = MD_REPORT_ID,
    ): ByteArray {
        val b = band.clamped()
        val pkt = ByteArray(64)
        pkt[0] = reportId
        pkt[1] = MD_CMD_WRITE
        pkt[2] = MD_CMD_UPDATE_EQ
        pkt[3] = 0x18
        pkt[4] = 0x00
        pkt[5] = index.toByte()
        pkt[6] = 0x00
        pkt[7] = 0x00

        val coeffs = BiquadMath.encodeMd8BiquadQ30(b)
        val bb = ByteBuffer.wrap(pkt).order(ByteOrder.LITTLE_ENDIAN)
        for (k in coeffs.indices) {
            bb.putInt(8 + k * 4, coeffs[k])
        }
        bb.putShort(28, b.frequency.roundToInt().toShort())
        bb.putShort(30, (b.q * 256.0).roundToInt().toShort())
        bb.putShort(32, (b.gain * 256.0).roundToInt().toShort())
        pkt[34] = mdTypeToCode(b.type)
        pkt[35] = 0
        pkt[36] = (slot and 0xFF).toByte()
        return pkt
    }

    fun md8BuildApplyPacket(index: Int, reportId: Byte = MD_REPORT_ID): ByteArray {
        val pkt = ByteArray(64)
        pkt[0] = reportId
        pkt[1] = MD_CMD_WRITE
        pkt[2] = MD_CMD_APPLY_COEFF
        pkt[3] = index.toByte()
        pkt[4] = 0
        pkt[5] = 0xFF.toByte()
        pkt[6] = 0xFF.toByte()
        pkt[7] = 0xFF.toByte()
        return pkt
    }

    fun md8BuildSaveFlashPacket(reportId: Byte = MD_REPORT_ID): ByteArray {
        val pkt = ByteArray(64)
        pkt[0] = reportId
        pkt[1] = MD_CMD_WRITE
        pkt[2] = MD_CMD_SAVE_FLASH
        return pkt
    }

    fun md8DecodeReadReply(reply: ByteArray): FilterBand? {
        val offset = if (reply.size > 1 && reply[0] == MD_REPORT_ID) 1 else 0
        if (reply.size < offset + 34) return null
        if (reply[offset] != MD_CMD_READ || reply[offset + 1] != MD_CMD_UPDATE_EQ) return null

        val bb = ByteBuffer.wrap(reply).order(ByteOrder.LITTLE_ENDIAN)
        val freq = (bb.getShort(offset + 27).toInt() and 0xFFFF).toDouble()
        val q = (bb.getShort(offset + 29).toInt() and 0xFFFF) / 256.0
        val gain = bb.getShort(offset + 31).toInt() / 256.0
        val ftype = mdCodeToType(reply[offset + 33].toInt() and 0xFF)
        return if (freq in 11.0..23999.0) {
            FilterBand(type = ftype, frequency = freq, gain = gain, q = q).clamped()
        } else {
            FilterBand(FilterType.PEAKING, 1000.0, 0.0, 1.0)
        }
    }

    fun wpBuildGlobalGainPacket(reportId: Byte, preampDb: Double): ByteArray {
        val p = preampDb.roundToInt().coerceIn(-12, 0).toByte()
        return byteArrayOf(reportId, MD_CMD_WRITE, WP_CMD_GLOBAL_GAIN, 0x01, p, 0x00)
    }

    fun wpBuildCommitPackets(reportId: Byte, saveFlash: Boolean): List<ByteArray> {
        val list = mutableListOf(
            byteArrayOf(reportId, MD_CMD_WRITE, 0x05, 0x00),
            byteArrayOf(reportId, MD_CMD_WRITE, 0x17, 0x00),
            byteArrayOf(
                reportId,
                MD_CMD_WRITE,
                MD_CMD_APPLY_COEFF,
                0x04,
                0x00,
                0x00,
                0xFF.toByte(),
                0xFF.toByte(),
                0x00,
            ),
        )
        if (saveFlash) {
            list += byteArrayOf(reportId, MD_CMD_WRITE, MD_CMD_SAVE_FLASH, 0x01, 0x00)
        }
        return list
    }

    // ---- 3. FiiO / JadeAudio / SNOWSKY (big-endian AA 0A / BB 0B ... EE) ----

    fun fiioBuildSetFilterPacket(reportId: Byte, index: Int, band: FilterBand): ByteArray {
        val b = band.clamped()
        val g = (b.gain * 10.0).roundToInt()
        val f = b.frequency.roundToInt()
        val q = (b.q * 100.0).roundToInt()
        return byteArrayOf(
            reportId,
            FIIO_SET_H1,
            FIIO_SET_H2,
            0x00,
            0x00,
            FIIO_CMD_FILTER_PARAMS,
            0x08,
            index.toByte(),
            ((g ushr 8) and 0xFF).toByte(),
            (g and 0xFF).toByte(),
            ((f ushr 8) and 0xFF).toByte(),
            (f and 0xFF).toByte(),
            ((q ushr 8) and 0xFF).toByte(),
            (q and 0xFF).toByte(),
            fiioTypeToCode(b.type),
            0x00,
            FIIO_END,
        )
    }

    fun fiioDecodeFilterPayload(payload: ByteArray): Pair<Int, FilterBand>? {
        if (payload.size < 14 ||
            payload[0] != FIIO_GET_H1 ||
            payload[1] != FIIO_GET_H2 ||
            payload[4] != FIIO_CMD_FILTER_PARAMS
        ) {
            return null
        }
        val idx = payload[6].toInt() and 0xFF
        val gRaw = (((payload[7].toInt() and 0xFF) shl 8) or (payload[8].toInt() and 0xFF)).toShort().toInt()
        val fRaw = ((payload[9].toInt() and 0xFF) shl 8) or (payload[10].toInt() and 0xFF)
        val qRaw = (((payload[11].toInt() and 0xFF) shl 8) or (payload[12].toInt() and 0xFF)) / 100.0
        val q = if (qRaw < 0.05) 1.0 else qRaw
        val ft = fiioCodeToType(payload[13].toInt() and 0xFF)
        val band = if (fRaw in 10..24000) {
            FilterBand(ft, fRaw.toDouble(), gRaw / 10.0, q).clamped()
        } else {
            FilterBand(FilterType.PEAKING, 1000.0, 0.0, 1.0)
        }
        return idx to band
    }

    fun fiioBuildGlobalGainPacket(reportId: Byte, preampDb: Double): ByteArray {
        val g = (preampDb.coerceIn(-12.0, 12.0) * 10.0).roundToInt()
        return byteArrayOf(
            reportId,
            FIIO_SET_H1,
            FIIO_SET_H2,
            0x00,
            0x00,
            FIIO_CMD_GLOBAL_GAIN,
            0x02,
            ((g ushr 8) and 0xFF).toByte(),
            (g and 0xFF).toByte(),
            0x00,
            FIIO_END,
        )
    }

    fun fiioBuildSimpleCmd(reportId: Byte, isSet: Boolean, cmd: Byte, arg: Int? = null): ByteArray {
        val h1 = if (isSet) FIIO_SET_H1 else FIIO_GET_H1
        val h2 = if (isSet) FIIO_SET_H2 else FIIO_GET_H2
        return if (arg != null) {
            byteArrayOf(reportId, h1, h2, 0x00, 0x00, cmd, 0x01, (arg and 0xFF).toByte(), 0x00, FIIO_END)
        } else {
            byteArrayOf(reportId, h1, h2, 0x00, 0x00, cmd, 0x00, 0x00, FIIO_END)
        }
    }

    // ---- 4. Fosi Audio DS3 (`0x77` Feature/Output float32 LE) ----

    fun fosiBuildCmdPacket(reportId: Byte, cmd: Byte, index: Int = 0): ByteArray {
        val pkt = ByteArray(64)
        pkt[0] = reportId
        pkt[1] = FOSI_HEADER
        pkt[2] = cmd
        pkt[3] = (index and 0xFF).toByte()
        return pkt
    }

    fun fosiEncodeBandPacket(reportId: Byte, presetId: Int, bandIndex: Int, band: FilterBand): ByteArray {
        val b = band.clamped()
        val pkt = ByteArray(64)
        pkt[0] = reportId
        pkt[1] = FOSI_HEADER
        pkt[2] = FOSI_CMD_SET_EQ_PARAMS
        pkt[3] = (presetId and 0xFF).toByte()
        pkt[4] = (bandIndex and 0xFF).toByte()
        pkt[5] = fosiTypeToCode(b.type)
        val bb = ByteBuffer.wrap(pkt).order(ByteOrder.LITTLE_ENDIAN)
        bb.putFloat(6, b.frequency.toFloat())
        bb.putFloat(10, b.q.toFloat())
        bb.putFloat(14, 0.0f)
        bb.putFloat(18, b.gain.toFloat())
        return pkt
    }

    fun fosiDecodeBandPacket(data: ByteArray): Triple<Int, FilterBand, Boolean>? {
        val off = when {
            data.isNotEmpty() && data[0] == FOSI_HEADER -> 0
            data.size > 1 && data[1] == FOSI_HEADER -> 1
            else -> return null
        }
        if (data.size < off + 21) return null
        val bandIdx = data[off + 3].toInt() and 0xFF
        val ftypeRaw = data[off + 4].toInt() and 0xFF
        val bb = ByteBuffer.wrap(data).order(ByteOrder.LITTLE_ENDIAN)
        val freq = bb.getFloat(off + 5).toDouble()
        val q = bb.getFloat(off + 9).toDouble()
        val gain = bb.getFloat(off + 17).toDouble()
        val disabled = ftypeRaw == 0 || (kotlin.math.abs(gain) < 1e-6 && freq <= 0.0)
        val band = if (!disabled && freq in 10.0..24000.0) {
            FilterBand(fosiCodeToType(ftypeRaw), freq, gain, q.coerceAtLeast(0.1)).clamped()
        } else {
            FilterBand(FilterType.PEAKING, 1000.0, 0.0, 1.0)
        }
        return Triple(bandIdx, band, disabled)
    }

    // ---- 5. Topping DX1 II / E50 II (16B `0x22 0x33 ... 0x66 0x77 0x00`) ----

    data class ToppingFrame(
        val protocolType: Int,
        val totalFrameLen: Int,
        val curFrame: Int,
        val cmd: Int,
        val dataU32: Long,
        val crc: Int,
    )

    fun toppingCrc16Modbus(frame16: ByteArray, offset: Int = 0): Int {
        var crc = 0xFFFF
        for (i in 2..10) {
            crc = crc xor (frame16[offset + i].toInt() and 0xFF)
            repeat(8) {
                crc = if ((crc and 1) != 0) (crc ushr 1) xor 0xA001 else (crc ushr 1)
            }
        }
        return crc and 0xFFFF
    }

    fun toppingBuildFrame(protocolType: Byte, cmd: Int, data: Int = 0): ByteArray {
        val pkt = ByteArray(17)
        pkt[0] = 0x00 // unnumbered reportId
        pkt[1] = 0x22
        pkt[2] = 0x33
        pkt[3] = protocolType
        pkt[4] = 0x01
        pkt[5] = 0x01
        pkt[6] = ((cmd ushr 8) and 0xFF).toByte()
        pkt[7] = (cmd and 0xFF).toByte()
        pkt[8] = ((data ushr 24) and 0xFF).toByte()
        pkt[9] = ((data ushr 16) and 0xFF).toByte()
        pkt[10] = ((data ushr 8) and 0xFF).toByte()
        pkt[11] = (data and 0xFF).toByte()
        pkt[14] = 0x66
        pkt[15] = 0x77
        pkt[16] = 0x00
        return pkt
    }

    fun toppingParseFrame(raw: ByteArray): ToppingFrame? {
        val off = when {
            raw.size >= 15 && raw[0] == 0x22.toByte() && raw[1] == 0x33.toByte() -> 0
            raw.size >= 16 && raw[0] == 0x00.toByte() && raw[1] == 0x22.toByte() && raw[2] == 0x33.toByte() -> 1
            else -> return null
        }
        if (raw.size < off + 15 || raw[off + 13] != 0x66.toByte() || raw[off + 14] != 0x77.toByte()) {
            return null
        }
        val cmd = ((raw[off + 5].toInt() and 0xFF) shl 8) or (raw[off + 6].toInt() and 0xFF)
        val dataU32 = ((raw[off + 7].toLong() and 0xFF) shl 24) or
            ((raw[off + 8].toLong() and 0xFF) shl 16) or
            ((raw[off + 9].toLong() and 0xFF) shl 8) or
            (raw[off + 10].toLong() and 0xFF)
        val crc = ((raw[off + 11].toInt() and 0xFF) shl 8) or (raw[off + 12].toInt() and 0xFF)
        return ToppingFrame(
            protocolType = raw[off + 2].toInt() and 0xFF,
            totalFrameLen = raw[off + 3].toInt() and 0xFF,
            curFrame = raw[off + 4].toInt() and 0xFF,
            cmd = cmd,
            dataU32 = dataU32,
            crc = crc,
        )
    }

    fun toppingEncodeGainByte(gainDb: Double): Int {
        val g = if (gainDb.isFinite()) gainDb.coerceIn(-12.0, 12.0) else 0.0
        return (g * 10.0).roundToInt().coerceIn(-128, 127)
    }

    fun toppingDecodePackedBand(packed: Long, freq: Long, qRaw: Long): Pair<Boolean, FilterBand> {
        val enabled = (packed and 0xFFL) == 1L
        val typeRaw = ((packed ushr 8) and 0xFFL).toInt()
        val gainRaw = ((packed ushr 16) and 0xFFL).toByte().toInt()
        val gain = gainRaw / 10.0
        val f = freq.toDouble()
        val q = qRaw.toDouble() / 10000.0
        val band = if (enabled && f in 10.0..24000.0) {
            FilterBand(toppingCodeToType(typeRaw), f, gain, q.coerceAtLeast(0.1)).clamped()
        } else {
            FilterBand(FilterType.PEAKING, 1000.0, 0.0, 0.707)
        }
        return enabled to band
    }

    // ---- 6. Qudelix-5K (`[payload_len+1, 0x80, cmd_hi, cmd_lo, ...data]`) ----

    fun qudelixBuildPacket(reportId: Byte, cmdId: Int, data: ByteArray = ByteArray(0)): ByteArray {
        val pkt = ByteArray(65)
        val payloadLen = 2 + data.size
        pkt[0] = reportId
        pkt[1] = (payloadLen + 1).toByte()
        pkt[2] = 0x80.toByte()
        pkt[3] = ((cmdId ushr 8) and 0xFF).toByte()
        pkt[4] = (cmdId and 0xFF).toByte()
        val n = minOf(data.size, 60)
        System.arraycopy(data, 0, pkt, 5, n)
        return pkt
    }

    fun qudelixBuildBandPacket(reportId: Byte, bandIdx: Int, band: FilterBand): ByteArray {
        val b = band.clamped()
        val freq = b.frequency.roundToInt().coerceIn(20, 20000)
        val gainScaled = (b.gain * 10.0).roundToInt()
        val qScaled = (b.q * 1024.0).roundToInt()
        val payload = byteArrayOf(
            0x00, // GROUP_USR
            0x03, // CH_MASK_BOTH
            (bandIdx and 0xFF).toByte(),
            qudelixTypeToCode(b.type),
            ((freq ushr 8) and 0xFF).toByte(),
            (freq and 0xFF).toByte(),
            ((gainScaled ushr 8) and 0xFF).toByte(),
            (gainScaled and 0xFF).toByte(),
            ((qScaled ushr 8) and 0xFF).toByte(),
            (qScaled and 0xFF).toByte(),
        )
        return qudelixBuildPacket(reportId, QUDELIX_CMD_SET_EQ_BAND_PARAM, payload)
    }

    private fun q5kReadBits(buf: ByteArray, bitOffset: Int, numBits: Int): Int {
        var value = 0
        for (i in 0 until numBits) {
            val pos = bitOffset + i
            val byteIdx = pos ushr 3
            if (byteIdx < buf.size && (((buf[byteIdx].toInt() ushr (pos and 7)) and 1) != 0)) {
                value = value or (1 shl i)
            }
        }
        return value
    }

    private fun q5kSignExtend10(v: Int): Int = ((v and 0x3FF) shl 22) shr 22

    fun qudelixParseUserEqPreset(buf: ByteArray): Pair<Double, List<FilterBand>> {
        require(buf.size >= 88) { "Buffer V3 de Qudelix-5K demasiado corto (${buf.size} bytes)." }
        var off = 0
        fun r(n: Int): Int {
            val v = q5kReadBits(buf, off, n)
            off += n
            return v
        }
        r(1); r(14); r(11); r(6)
        val preGain0 = r(16).toShort().toInt()
        r(16)

        val freqsCh0 = IntArray(10) { r(16) }
        repeat(10) { r(16) }

        val defaultFreqs = doubleArrayOf(32.0, 64.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0)
        val bands = List(10) { idx ->
            val typeRaw = r(4)
            val gainRaw = r(10)
            val qRaw = r(14)
            r(4)
            val freq = if (freqsCh0[idx] > 0) freqsCh0[idx].toDouble() else defaultFreqs[idx]
            val q = if (qRaw > 0) ((qRaw / 1024.0) * 100.0).roundToInt() / 100.0 else 1.0
            val gain = if (typeRaw == 0) 0.0 else q5kSignExtend10(gainRaw) / 10.0
            FilterBand(qudelixCodeToType(typeRaw), freq, gain, q).clamped()
        }
        return (preGain0 / 10.0) to bands
    }

    // ---- 7. JDS Labs Element IV (USB-CDC Serial JSON 12-band) ----

    fun jdsBuildDescribePayload(): ByteArray {
        val obj = buildJsonObject {
            put("Product", "JDS Labs Element IV")
            put("Action", "Describe")
        }
        return obj.toString().toByteArray(Charsets.UTF_8) + byteArrayOf(0)
    }

    fun jdsAlign12Bands(bands: List<FilterBand>): List<FilterBand> {
        val ls = mutableListOf<FilterBand>()
        val pk = mutableListOf<FilterBand>()
        val hs = mutableListOf<FilterBand>()
        for (b in bands) {
            val c = b.clamped(maxGainDb = 12.0, allowShelves = true)
            when (c.type) {
                FilterType.LOW_SHELF -> if (ls.size < 2) ls += c
                FilterType.HIGH_SHELF -> if (hs.size < 2) hs += c
                FilterType.PEAKING -> if (pk.size < 8) pk += c
            }
        }
        val aligned = ArrayList<FilterBand>(12)
        for (i in 0 until 2) aligned += ls.getOrNull(i) ?: FilterBand(FilterType.LOW_SHELF, 80.0, 0.0, 0.707)
        for (i in 0 until 8) aligned += pk.getOrNull(i) ?: FilterBand(FilterType.PEAKING, 1000.0, 0.0, 0.707)
        for (i in 0 until 2) aligned += hs.getOrNull(i) ?: FilterBand(FilterType.HIGH_SHELF, 10000.0, 0.0, 0.707)
        return aligned
    }

    private fun jdsTypeString(t: FilterType): String = when (t) {
        FilterType.LOW_SHELF -> "LOWSHELF"
        FilterType.PEAKING -> "PEAKING"
        FilterType.HIGH_SHELF -> "HIGHSHELF"
    }

    private fun jdsParseType(raw: String): FilterType = when (raw.trim().uppercase()) {
        "LOWSHELF", "LSQ" -> FilterType.LOW_SHELF
        "HIGHSHELF", "HSQ" -> FilterType.HIGH_SHELF
        else -> FilterType.PEAKING
    }

    fun jdsBuildUpdatePayload(bands: List<FilterBand>, preampDb: Double, enableEq: Boolean): ByteArray {
        val aligned = jdsAlign12Bands(bands)
        val effPreamp = if (enableEq) (preampDb * 10.0).roundToInt() / 10.0 else 0.0
        val headphone = buildJsonObject {
            put(
                "Preamp",
                buildJsonObject {
                    put("Gain", effPreamp)
                    put("Mode", "AUTO")
                },
            )
            JDS_12_BAND_ORDER.forEachIndexed { idx, name ->
                val b = aligned[idx]
                val g = if (enableEq) (b.gain * 10.0).roundToInt() / 10.0 else 0.0
                put(
                    name,
                    buildJsonObject {
                        put("Gain", g)
                        put("Frequency", b.frequency.roundToInt())
                        put("Q", (b.q * 1000.0).roundToInt() / 1000.0)
                        put("Type", jdsTypeString(b.type))
                    },
                )
            }
        }
        val payload = buildJsonObject {
            put("Product", "JDS Labs Element IV")
            put("FormatOutput", true)
            put("Action", "Update")
            put(
                "Configuration",
                buildJsonObject {
                    put(
                        "DSP",
                        buildJsonObject {
                            put("Headphone", headphone)
                        },
                    )
                },
            )
        }
        return payload.toString().toByteArray(Charsets.UTF_8) + byteArrayOf(0)
    }

    private fun jdsFieldDouble(obj: JsonObject, key: String, fallback: Double): Double {
        val el = obj[key] ?: return fallback
        return if (el is JsonObject) {
            el["Current"]?.jsonPrimitive?.doubleOrNull ?: fallback
        } else {
            (el as? JsonPrimitive)?.doubleOrNull ?: fallback
        }
    }

    fun jdsParseDescribeResponse(jsonStr: String): Pair<Double, List<FilterBand>> {
        val clean = jsonStr.trim('\u0000').trim()
        val root = Json.parseToJsonElement(clean).jsonObject
        val cfg = root["Configuration"]?.jsonObject
            ?: throw IllegalArgumentException("Respuesta Describe sin Configuration")
        val dsp = cfg["DSP"]?.jsonObject
            ?: throw IllegalArgumentException("Respuesta Describe sin Configuration.DSP")
        val hp = dsp["Headphone"]?.jsonObject
            ?: throw IllegalArgumentException("Respuesta Describe sin Configuration.DSP.Headphone")

        val preamp = hp["Preamp"]?.jsonObject?.let { p -> jdsFieldDouble(p, "Gain", 0.0) } ?: 0.0
        val bands = JDS_12_BAND_ORDER.map { name ->
            val fobj = hp[name]?.jsonObject
            if (fobj != null) {
                val tEl = fobj["Type"]
                val typeStr = when (tEl) {
                    is JsonObject -> tEl["Current"]?.jsonPrimitive?.content
                    is JsonPrimitive -> tEl.content
                    else -> null
                } ?: when {
                    name.startsWith("Lowshelf") -> "LOWSHELF"
                    name.startsWith("Highshelf") -> "HIGHSHELF"
                    else -> "PEAKING"
                }
                val freq = jdsFieldDouble(fobj, "Frequency", 1000.0)
                val gain = jdsFieldDouble(fobj, "Gain", 0.0)
                val q = jdsFieldDouble(fobj, "Q", 0.707)
                FilterBand(jdsParseType(typeStr), freq, gain, q).clamped()
            } else when {
                name.startsWith("Lowshelf") -> FilterBand(FilterType.LOW_SHELF, 80.0, 0.0, 0.707)
                name.startsWith("Highshelf") -> FilterBand(FilterType.HIGH_SHELF, 10000.0, 0.0, 0.707)
                else -> FilterBand(FilterType.PEAKING, 1000.0, 0.0, 0.707)
            }
        }
        return preamp to bands
    }

    // ---- 8. Bluetooth Classic SPP (RFCOMM) framing ----

    fun sppFrame(payload: ByteArray): ByteArray {
        val n = (payload.size - 4).coerceAtLeast(0)
        val out = ByteArray(4 + payload.size)
        out[0] = 0xFF.toByte()
        out[1] = 0x04
        out[2] = ((n ushr 8) and 0xFF).toByte()
        out[3] = (n and 0xFF).toByte()
        System.arraycopy(payload, 0, out, 4, payload.size)
        return out
    }

    fun sppPresetPayload(presetIdx: Int): ByteArray =
        byteArrayOf(0x00, 0x1D, 0x0A, 0x03, (presetIdx and 0xFF).toByte())

    fun sppReadEqPayload(): ByteArray =
        byteArrayOf(0x00, 0x1D, 0x0A, 0x05)

    fun sppWriteEqPayload(bands: List<FilterBand>): ByteArray {
        val five = bands.take(5).map { it.clamped() }.toMutableList()
        while (five.size < 5) {
            five += FilterBand(FilterType.PEAKING, 1000.0, 0.0, 1.0)
        }
        val maxPos = five.filter { it.gain > 0.0 }.maxOfOrNull { it.gain } ?: 0.0
        val preamp = -maxPos

        val body = ByteArray(4 + five.size * 7)
        body[0] = 0x00
        body[1] = 0x04

        fun putS16(offset: Int, db: Double) {
            val r = (db * 60.0).roundToInt().coerceIn(-32768, 32767)
            body[offset] = ((r ushr 8) and 0xFF).toByte()
            body[offset + 1] = (r and 0xFF).toByte()
        }

        fun putU16(offset: Int, v: Int) {
            val r = v.coerceIn(0, 65535)
            body[offset] = ((r ushr 8) and 0xFF).toByte()
            body[offset + 1] = (r and 0xFF).toByte()
        }

        putS16(2, preamp)
        for (i in five.indices) {
            val o = 2 + i * 7
            putS16(o, if (i == 0) preamp else five[i - 1].gain)
            putU16(o + 2, five[i].frequency.roundToInt())
            putU16(o + 4, (five[i].q * 4096.0).roundToInt())
            body[o + 6] = 0
        }
        putS16(2 + five.size * 7, five.last().gain)

        val header = byteArrayOf(0x00, 0x1D, 0x0A, 0x06)
        return header + body
    }

    fun sppDecodeEqFrame(frame: ByteArray): List<FilterBand>? {
        var idx = -1
        for (i in 6 until frame.size - 1) {
            if (frame[i] == 0x00.toByte() && frame[i + 1] == 0x04.toByte()) {
                idx = i
                break
            }
        }
        if (idx < 0) return null
        val p = frame.copyOfRange(idx, frame.size)
        if (p.size < 11 || (p.size - 4) % 7 != 0) return null
        val n = (p.size - 4) / 7

        fun s16(o: Int): Int {
            val hi = p[o].toInt() and 0xFF
            val lo = p[o + 1].toInt() and 0xFF
            return ((hi shl 8) or lo).toShort().toInt()
        }

        fun u16(o: Int): Int {
            val hi = p[o].toInt() and 0xFF
            val lo = p[o + 1].toInt() and 0xFF
            return (hi shl 8) or lo
        }

        return List(n) { i ->
            val o = 2 + i * 7
            val gRaw = if (i < n - 1) s16(o + 7) else s16(2 + n * 7)
            FilterBand(
                type = FilterType.PEAKING,
                frequency = u16(o + 2).toDouble(),
                gain = (gRaw / 60.0),
                q = (u16(o + 4) / 4096.0),
            ).clamped()
        }
    }
}
