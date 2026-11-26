package com.hector.moondropdsp.hardware

import com.hector.moondropdsp.dsp.BiquadMath
import com.hector.moondropdsp.dsp.FilterBand
import com.hector.moondropdsp.dsp.FilterType
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.math.roundToInt

/**
 * Pure JVM protocol codecs matching chu2_hid.py (KTMicro 5-band & Moondrop 8-band USB HID)
 * and moondrop_bt_pw.py (Moondrop Bluetooth Classic SPP RFCOMM).
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

    // ---- Moondrop 8-band USB HID constants ----
    const val MD_REPORT_ID: Byte = 0x4B
    const val MD_CMD_WRITE: Byte = 0x01
    const val MD_CMD_READ: Byte = 0x80.toByte()
    const val MD_CMD_UPDATE_EQ: Byte = 0x09
    const val MD_CMD_APPLY_COEFF: Byte = 0x0A
    const val MD_CMD_SAVE_FLASH: Byte = 0x01

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
        // Some Android USB interrupt transfers omit the reportId prefix when report ID is implicit
        if (reply.size >= 10 && reply[0] == reg && reply[4] == cmd) {
            return reply.copyOfRange(6, 10)
        }
        return null
    }

    fun md8BuildReadPacket(index: Int): ByteArray {
        val pkt = ByteArray(64)
        pkt[0] = MD_REPORT_ID
        pkt[1] = MD_CMD_READ
        pkt[2] = MD_CMD_UPDATE_EQ
        pkt[3] = 0x18
        pkt[4] = 0x00
        pkt[5] = index.toByte()
        pkt[6] = 0x00
        return pkt
    }

    fun md8BuildWritePacket(index: Int, band: FilterBand): ByteArray {
        val b = band.clamped()
        val pkt = ByteArray(64)
        pkt[0] = MD_REPORT_ID
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
        pkt[36] = 7
        return pkt
    }

    fun md8BuildApplyPacket(index: Int): ByteArray {
        val pkt = ByteArray(64)
        pkt[0] = MD_REPORT_ID
        pkt[1] = MD_CMD_WRITE
        pkt[2] = MD_CMD_APPLY_COEFF
        pkt[3] = index.toByte()
        pkt[4] = 0
        pkt[5] = 0xFF.toByte()
        pkt[6] = 0xFF.toByte()
        pkt[7] = 0xFF.toByte()
        return pkt
    }

    fun md8BuildSaveFlashPacket(): ByteArray {
        val pkt = ByteArray(64)
        pkt[0] = MD_REPORT_ID
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

    // ---- Bluetooth Classic SPP (RFCOMM) framing matching moondrop_bt_pw.py ----

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
