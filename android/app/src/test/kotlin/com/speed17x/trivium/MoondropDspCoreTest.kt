package com.speed17x.trivium

import com.speed17x.trivium.data.AppPreferencesStore
import com.speed17x.trivium.data.AutoEqParser
import com.speed17x.trivium.data.SquigSourceSpec
import com.speed17x.trivium.dsp.BiquadMath
import com.speed17x.trivium.dsp.FilterBand
import com.speed17x.trivium.dsp.FilterType
import com.speed17x.trivium.hardware.DspProtocol
import com.speed17x.trivium.hardware.MoondropDeviceCatalog
import com.speed17x.trivium.hardware.MoondropProtocolCodec
import com.speed17x.trivium.ui.theme.AppAccentId
import com.speed17x.trivium.ui.theme.AppThemeId
import com.speed17x.trivium.ui.theme.tokensFor
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.math.abs

class MoondropDspCoreTest {

    @Test
    fun builtInPresetsExcludeChu2FactoryAndDefaultTo10Bands() {
        val names = AppPreferencesStore.BUILT_IN_PRESETS.map { it.name.lowercase() }
        assertFalse("Chu 2 factory curve must not be in built-in presets", names.any { "chu 2" in it || "chu2" in it })
        assertEquals(10, BiquadMath.DEFAULT_VIRTUAL_BANDS)
        assertEquals(10, BiquadMath.flatBands().size)
    }

    @Test
    fun peakingBandCenterGainMatchesTarget() {
        val band = FilterBand(FilterType.PEAKING, 1000.0, 6.0, 1.414)
        val resp = BiquadMath.bandResponseDb(band, doubleArrayOf(20.0, 1000.0, 18000.0))
        assertEquals(0.0, resp[0], 0.05)
        assertEquals(6.0, resp[1], 0.05)
        assertEquals(0.0, resp[2], 0.1)
    }

    @Test
    fun shelvesBoostProperSides() {
        val ls = FilterBand(FilterType.LOW_SHELF, 105.0, 5.0, 0.707)
        val lsResp = BiquadMath.bandResponseDb(ls, doubleArrayOf(20.0, 10000.0))
        assertTrue("Low shelf should boost sub-bass", lsResp[0] > 4.5)
        assertTrue("Low shelf should stay flat in treble", abs(lsResp[1]) < 0.1)

        val hs = FilterBand(FilterType.HIGH_SHELF, 8000.0, -4.0, 0.707)
        val hsResp = BiquadMath.bandResponseDb(hs, doubleArrayOf(50.0, 16000.0))
        assertTrue("High shelf should stay flat in bass", abs(hsResp[0]) < 0.1)
        assertTrue("High shelf should cut upper treble", hsResp[1] < -2.5)
    }

    @Test
    fun peqOptimizer10BandConvergesTightly() {
        val freqs = BiquadMath.logSpace(20.0, 20000.0, 128)
        val referenceBands = listOf(
            FilterBand(FilterType.LOW_SHELF, 105.0, 4.0, 0.707),
            FilterBand(FilterType.PEAKING, 250.0, -2.5, 1.2),
            FilterBand(FilterType.PEAKING, 1400.0, 1.8, 1.5),
            FilterBand(FilterType.PEAKING, 3200.0, 3.0, 2.0),
            FilterBand(FilterType.PEAKING, 6400.0, -3.2, 3.5),
        )
        val targetCurve = BiquadMath.combinedResponseDb(referenceBands, freqs)

        val (fitted, rmse) = BiquadMath.fitPeqBands(
            freqs = freqs,
            targetDb = targetCurve,
            maxBands = 10,
            maxFreqFit = 10000.0,
            maxGainDb = 12.0,
            supportsShelves = true,
        )
        assertEquals(10, fitted.size)
        assertTrue("Expected 10-band RMSE < 0.20 dB, got $rmse", rmse < 0.20)
    }

    @Test
    fun ktMicroEncodeDecodeRoundTrip() {
        val original = FilterBand(FilterType.LOW_SHELF, 105.0, -3.4, 0.707)
        val (gf, qt) = MoondropProtocolCodec.ktEncodeBand(original)
        val decoded = MoondropProtocolCodec.ktDecodeBand(gf, qt)
        assertEquals(FilterType.LOW_SHELF, decoded.type)
        assertEquals(105.0, decoded.frequency, 0.01)
        assertEquals(-3.4, decoded.gain, 0.01)
        assertEquals(0.707, decoded.q, 0.002)

        val pkt = MoondropProtocolCodec.ktPacket(MoondropProtocolCodec.KT_REG_BAND0, MoondropProtocolCodec.KT_WRITE, gf)
        assertEquals(11, pkt.size)
        assertEquals(0x4B.toByte(), pkt[0])
        assertEquals(0x26.toByte(), pkt[1])
        assertEquals(0x57.toByte(), pkt[5])
        assertArrayEquals(gf, MoondropProtocolCodec.ktExtractReplyValue(pkt, MoondropProtocolCodec.KT_REG_BAND0, MoondropProtocolCodec.KT_WRITE))
    }

    @Test
    fun moondrop8WriteReadRoundTrip() {
        val original = FilterBand(FilterType.HIGH_SHELF, 6500.0, 2.5, 1.25)
        val writePkt = MoondropProtocolCodec.md8BuildWritePacket(3, original)
        assertEquals(64, writePkt.size)
        assertEquals(0x4B.toByte(), writePkt[0])
        assertEquals(0x01.toByte(), writePkt[1])
        assertEquals(0x09.toByte(), writePkt[2])

        val reply = writePkt.copyOf()
        reply[1] = MoondropProtocolCodec.MD_CMD_READ
        val decoded = MoondropProtocolCodec.md8DecodeReadReply(reply)
        assertNotNull(decoded)
        assertEquals(FilterType.HIGH_SHELF, decoded!!.type)
        assertEquals(6500.0, decoded.frequency, 1.0)
        assertEquals(2.5, decoded.gain, 0.1)
        assertEquals(1.25, decoded.q, 0.02)
    }

    @Test
    fun bluetoothSppFrameRoundTrip() {
        val bands = listOf(
            FilterBand(FilterType.PEAKING, 60.0, 3.0, 0.8),
            FilterBand(FilterType.PEAKING, 250.0, -1.5, 1.2),
            FilterBand(FilterType.PEAKING, 1000.0, 0.0, 1.0),
            FilterBand(FilterType.PEAKING, 3500.0, 2.0, 1.8),
            FilterBand(FilterType.PEAKING, 8000.0, -2.5, 2.2),
        )
        val payload = MoondropProtocolCodec.sppWriteEqPayload(bands)
        val framed = MoondropProtocolCodec.sppFrame(payload)
        assertEquals(0xFF.toByte(), framed[0])
        assertEquals(0x04.toByte(), framed[1])

        val decoded = MoondropProtocolCodec.sppDecodeEqFrame(framed)
        assertNotNull(decoded)
        assertEquals(5, decoded!!.size)
        for (i in 0 until 5) {
            assertEquals(bands[i].frequency, decoded[i].frequency, 1.0)
            assertEquals(bands[i].gain, decoded[i].gain, 0.15)
            assertEquals(bands[i].q, decoded[i].q, 0.01)
        }
    }

    @Test
    fun squiglinkPhoneBookAndRewTxtParsersWork() {
        val phoneBook = """
            [
              {
                "name": "7Hz",
                "phones": [
                  {
                    "name": "x Crinacle Zero 3",
                    "file": ["7Hz Zero 3", "7Hz Zero 3 deep"],
                    "suffix": ["", "(insert: deep)"]
                  }
                ]
              },
              {
                "name": "Moondrop",
                "phones": [
                  {
                    "name": "Golden Ages 2",
                    "file": ["Moondrop Golden Ages 2 pop", "Moondrop Golden Ages 2 reference"],
                    "suffix": ["(preset: pop)", "(preset: reference)"]
                  }
                ]
              }
            ]
        """.trimIndent()
        val src = SquigSourceSpec("superreview", "Super* Review (711)", "https://squig.link/data/")
        val parsed = AutoEqParser.parseSquigPhoneBook(phoneBook, src)
        assertEquals(4, parsed.size)
        assertEquals("7Hz x Crinacle Zero 3", parsed[0].name)
        assertEquals("7Hz x Crinacle Zero 3 (insert: deep)", parsed[1].name)
        assertEquals("Moondrop Golden Ages 2 (preset: pop)", parsed[2].name)

        // Punctuation-tolerant search matches both "zero 3" and "zero:3"
        val matchZero3 = AutoEqParser.findBestEntryAnyBrand(parsed, "7Hz x Crinacle Zero:3")
        assertNotNull(matchZero3)
        assertTrue(matchZero3!!.name.contains("Zero 3"))

        val matchGa2 = AutoEqParser.findBestEntryAnyBrand(parsed, "Golden Ages 2")
        assertNotNull(matchGa2)
        assertTrue(matchGa2!!.name.contains("Golden Ages 2"))

        val rewTxt = listOf(
            "* Measurement data measured by REW V5.40",
            "* Freq(Hz) SPL(dB) Phase(deg)",
            "20.0\t105.0\t-170.0",
            "40.0\t104.2\t-150.0",
            "80.0\t102.0\t-120.0",
            "160.0\t98.5\t-80.0",
            "320.0\t96.0\t-40.0",
            "640.0\t95.5\t-10.0",
            "1000.0\t96.0\t0.0",
            "2000.0\t101.0\t20.0",
            "4000.0\t103.5\t30.0",
            "8000.0\t97.0\t10.0",
            "16000.0\t88.0\t-5.0",
        ).joinToString("\n")
        val (freqs, dbs) = AutoEqParser.parseSquigFrTxt(rewTxt)
        assertEquals(11, freqs.size)
        assertEquals(20.0, freqs.first(), 0.01)
        assertEquals(16000.0, freqs.last(), 0.01)
        assertEquals(96.0, dbs[6], 0.01)

        // RFC-3986 path segment encoding
        assertEquals("Moondrop%20Golden%20Ages%202%20L.txt", AutoEqParser.encodeUrlPathSegment("Moondrop Golden Ages 2 L.txt"))
        assertEquals("Zero%3A3%20L.txt", AutoEqParser.encodeUrlPathSegment("Zero:3 L.txt"))
    }

    @Test
    fun autoEqParsersAndDeviceCatalogWork() {
        val indexMd = """
            - [Moondrop Chu 2](./crinacle/711%20in-ear/Moondrop%20Chu%202) by crinacle on 711
            - [Truthear Hexa](./crinacle/711%20in-ear/Truthear%20Hexa) by crinacle on 711
            - [Sony WH-1000XM5](./oratory1990/over-ear/Sony%20WH-1000XM5) by oratory1990
            - [Nothing Ear (2)](./crinacle/711%20in-ear/Nothing%20Ear%20(2)) by crinacle on 711
        """.trimIndent()
        val entries = AutoEqParser.parseIndexMarkdown(indexMd)
        assertEquals(4, entries.size)

        val matchedSony = AutoEqParser.findBestEntryAnyBrand(entries, MoondropDeviceCatalog.inferAutoEqHint("Sony WH-1000XM5 Stereo"))
        assertNotNull(matchedSony)
        assertEquals("Sony WH-1000XM5", matchedSony!!.name)

        val apo = AutoEqParser.exportParametricEq(BiquadMath.HARMAN_WARM_BANDS)
        val reparsed = AutoEqParser.parseParametricEq(apo)
        assertEquals(BiquadMath.HARMAN_WARM_BANDS.size, reparsed.size)
        assertEquals(105.0, reparsed[0].frequency, 0.1)

        val chu2 = MoondropDeviceCatalog.classifyUsb("/dev/bus/usb/001/002", 0x31B2, 0x0113, "Chu2 DSP", true)
        assertNotNull(chu2)
        assertEquals("CHU 2 DSP", chu2!!.shortWidgetName)
        assertEquals(5, chu2.maxBands)
        assertTrue(MoondropDeviceCatalog.isMoondropBluetoothName("MOONDROP Space Travel 2 Ultra"))
        assertTrue(MoondropDeviceCatalog.isMoondropBluetoothName("Golden Ages 2"))
    }

    @Test
    fun classifiesAll8UsbDspFamiliesWithoutFalsePositives() {
        val ja11 = MoondropDeviceCatalog.classifyUsb("/dev/usb/1", 0x2972, 0x0011, "JadeAudio JA11", true)!!
        assertEquals(DspProtocol.FIIO, ja11.protocol)
        assertEquals(5, ja11.maxBands)
        assertEquals(2, ja11.reportId)

        val ka17 = MoondropDeviceCatalog.classifyUsb("/dev/usb/2", 0x2972, 0x0017, "FIIO KA17", true)!!
        assertEquals(DspProtocol.FIIO, ka17.protocol)
        assertEquals(10, ka17.maxBands)

        val pmax = MoondropDeviceCatalog.classifyUsb("/dev/usb/3", 0x3302, 0x43CC, "CrinEar Protocol Max", true)!!
        assertEquals(DspProtocol.WALKPLAY, pmax.protocol)
        assertEquals(10, pmax.maxBands)
        assertEquals(101, pmax.userSlot)

        val q5k = MoondropDeviceCatalog.classifyUsb("/dev/usb/4", 0x0A12, 0x4125, "Qudelix-5K USB DAC", true)!!
        assertEquals(DspProtocol.QUDELIX_5K, q5k.protocol)
        assertEquals(10, q5k.maxBands)

        val top = MoondropDeviceCatalog.classifyUsb("/dev/usb/5", 0x152A, 0x8750, "TOPPING DX1 II", true)!!
        assertEquals(DspProtocol.TOPPING, top.protocol)
        assertEquals(11, top.maxBands)

        val fosi = MoondropDeviceCatalog.classifyUsb("/dev/usb/6", 0x152A, 0x8700, "Fosi Audio DS3", true)!!
        assertEquals(DspProtocol.FOSI_DS3, fosi.protocol)
        assertEquals(8, fosi.maxBands)

        val jds = MoondropDeviceCatalog.classifyUsb("/dev/usb/7", 0x152A, 0x88FA, "JDS Labs Element IV", true)!!
        assertEquals(DspProtocol.JDS_ELEMENT, jds.protocol)
        assertEquals(12, jds.maxBands)

        // Generic 0x152A device without Topping/Fosi/Element signature must not match
        val generic = MoondropDeviceCatalog.classifyUsb("/dev/usb/8", 0x152A, 0x1234, "Generic XMOS Audio", true)
        assertEquals(null, generic)
    }

    @Test
    fun fiioFosiToppingQudelixAndJdsCodecsRoundTrip() {
        // 1. FiiO
        val band = FilterBand(FilterType.HIGH_SHELF, 8500.0, -4.5, 1.41)
        val fiioPkt = MoondropProtocolCodec.fiioBuildSetFilterPacket(7, 3, band)
        assertEquals(7.toByte(), fiioPkt[0])
        assertEquals(0xAA.toByte(), fiioPkt[1])
        val fiioResp = fiioPkt.copyOfRange(1, fiioPkt.size)
        fiioResp[0] = MoondropProtocolCodec.FIIO_GET_H1
        fiioResp[1] = MoondropProtocolCodec.FIIO_GET_H2
        val (fIdx, fBand) = MoondropProtocolCodec.fiioDecodeFilterPayload(fiioResp)!!
        assertEquals(3, fIdx)
        assertEquals(FilterType.HIGH_SHELF, fBand.type)
        assertEquals(8500.0, fBand.frequency, 0.1)
        assertEquals(-4.5, fBand.gain, 0.01)

        // 2. Fosi DS3 (float32 LE)
        val fosiBand = FilterBand(FilterType.LOW_SHELF, 105.0, -3.5, 0.707)
        val fosiPkt = MoondropProtocolCodec.fosiEncodeBandPacket(1, 7, 2, fosiBand)
        val (dIdx, dBand, disabled) = MoondropProtocolCodec.fosiDecodeBandPacket(fosiPkt)!!
        assertFalse(disabled)
        assertEquals(2, dIdx)
        assertEquals(FilterType.LOW_SHELF, dBand.type)
        assertEquals(105.0, dBand.frequency, 0.01)
        assertEquals(-3.5, dBand.gain, 0.01)

        // 3. Topping 16B frame + CRC16
        val tPkt = MoondropProtocolCodec.toppingBuildFrame(MoondropProtocolCodec.TOPPING_PTYPE_WRITE_NACK, 0x9E01, 0)
        val tParsed = MoondropProtocolCodec.toppingParseFrame(tPkt)!!
        assertEquals(0x9E01, tParsed.cmd)
        assertTrue(MoondropProtocolCodec.toppingCrc16Modbus(tPkt, 1) != 0)
        val gainByte = MoondropProtocolCodec.toppingEncodeGainByte(-4.2) and 0xFF
        val packed = 1L or (2L shl 8) or (gainByte.toLong() shl 16)
        val (tEn, tBand) = MoondropProtocolCodec.toppingDecodePackedBand(packed, 105L, 7070L)
        assertTrue(tEn)
        assertEquals(FilterType.LOW_SHELF, tBand.type)
        assertEquals(-4.2, tBand.gain, 0.01)

        // 4. Qudelix-5K packet + V3 88-byte bitstream
        val qPkt = MoondropProtocolCodec.qudelixBuildBandPacket(8, 3, band)
        assertEquals(8.toByte(), qPkt[0])
        assertEquals(0x80.toByte(), qPkt[2])
        val v3 = ByteArray(88)
        fun writeBits(bitOff: Int, n: Int, v: Int) {
            for (i in 0 until n) {
                if (((v ushr i) and 1) != 0) {
                    val pos = bitOff + i
                    v3[pos ushr 3] = (v3[pos ushr 3].toInt() or (1 shl (pos and 7))).toByte()
                }
            }
        }
        writeBits(32, 16, (-30).toShort().toInt() and 0xFFFF)
        writeBits(64, 16, 1000)
        writeBits(384, 4, 5) // Peak
        writeBits(388, 10, 30) // +3.0 dB
        writeBits(398, 14, 1024) // Q=1.0
        val (qPre, qBands) = MoondropProtocolCodec.qudelixParseUserEqPreset(v3)
        assertEquals(-3.0, qPre, 0.01)
        assertEquals(10, qBands.size)
        assertEquals(FilterType.PEAKING, qBands[0].type)
        assertEquals(3.0, qBands[0].gain, 0.01)

        // 5. JDS Labs Element IV (12-band align + JSON Describe/Update)
        val aligned = MoondropProtocolCodec.jdsAlign12Bands(listOf(fosiBand, FilterBand(FilterType.PEAKING, 250.0, -2.0, 1.4), band))
        assertEquals(12, aligned.size)
        assertEquals(FilterType.LOW_SHELF, aligned[0].type)
        assertEquals(FilterType.PEAKING, aligned[2].type)
        assertEquals(FilterType.HIGH_SHELF, aligned[10].type)
        val upd = MoondropProtocolCodec.jdsBuildUpdatePayload(aligned, -3.5, true)
        assertEquals(0.toByte(), upd.last())
        val descJson = """
            {"Product":"JDS Labs Element IV","Configuration":{"DSP":{"Headphone":{"Preamp":{"Gain":{"Current":-3.5}},"Lowshelf 1":{"Type":{"Current":"LOWSHELF"},"Frequency":{"Current":105},"Gain":{"Current":3.2},"Q":{"Current":0.707}},"Peaking 1":{"Type":{"Current":"PEAKING"},"Frequency":{"Current":1000},"Gain":{"Current":-2.4},"Q":{"Current":1.5}},"Highshelf 1":{"Type":{"Current":"HIGHSHELF"},"Frequency":{"Current":10000},"Gain":{"Current":1.6},"Q":{"Current":0.707}}}}}}
        """.trimIndent()
        val (jPre, jBands) = MoondropProtocolCodec.jdsParseDescribeResponse(descJson)
        assertEquals(-3.5, jPre, 0.01)
        assertEquals(12, jBands.size)
        assertEquals(3.2, jBands[0].gain, 0.01)
        assertEquals(-2.4, jBands[2].gain, 0.01)
        assertEquals(1.6, jBands[10].gain, 0.01)
    }

    @Test
    fun modularThemesHaveUpdatedNamesAndAccentOverride() {
        assertEquals("Default", AppThemeId.PURE_HYPER.label)
        assertEquals("Default claro", AppThemeId.PURE_LIGHT.label)
        assertEquals("NothingOS", AppThemeId.NOTHING_OS.label)

        val custom = tokensFor(AppThemeId.PURE_HYPER, AppAccentId.EMERALD)
        assertEquals(AppAccentId.EMERALD.darkColor, custom.accent)

        val lightCustom = tokensFor(AppThemeId.PURE_LIGHT, AppAccentId.PURPLE)
        assertEquals(AppAccentId.PURPLE.lightColor, lightCustom.accent)
    }

    @Test
    fun preampHeadroomAndMultiFormatExportersWork() {
        val bands = listOf(
            FilterBand(FilterType.LOW_SHELF, 105.0, 4.0, 0.707),
            FilterBand(FilterType.PEAKING, 1000.0, 2.5, 1.4),
            FilterBand(FilterType.HIGH_SHELF, 8500.0, -2.0, 0.707),
        )
        val peak = BiquadMath.peakBoostDb(bands)
        assertTrue("Expected positive peak boost, got $peak", peak > 3.5)
        val safe = BiquadMath.computeSafePreampDb(bands)
        assertTrue(BiquadMath.headroomMarginDb(bands, safe) >= 0.0)
        assertTrue(BiquadMath.headroomMarginDb(bands, 0.0) < -3.0)

        val ee = AutoEqParser.exportEasyEffectsJson(bands, safe)
        assertTrue(ee.contains("\"equalizer#0\""))
        assertTrue(ee.contains("\"Lo-shelf\""))
        assertTrue(ee.contains("\"num-bands\": 3"))

        val cdsp = AutoEqParser.exportCamillaDspYaml(bands, safe)
        assertTrue(cdsp.contains("type: Gain"))
        assertTrue(cdsp.contains("type: Lowshelf"))
        assertTrue(cdsp.contains("pipeline:"))

        val wavelet = AutoEqParser.exportWaveletGraphicEq(bands)
        assertTrue(wavelet.startsWith("GraphicEQ: 20 "))
        val pairs = wavelet.trim().removePrefix("GraphicEQ:").split(";")
        assertEquals(127, pairs.size)
        for (p in pairs) {
            val db = p.trim().split(Regex("\\s+"))[1].toDouble()
            assertTrue("Wavelet GraphicEQ peak must stay <= 0 dB, got $db", db <= 0.01)
        }
    }

    @Test
    fun conexant9BandAndBleGattCodecsWork() {
        val freedsp = MoondropDeviceCatalog.classifyUsb("/dev/usb/9", 0x35D8, 0x1496, "Moondrop FreeDSP", true)!!
        assertEquals(DspProtocol.CONEXANT_9B, freedsp.protocol)
        assertEquals(9, freedsp.maxBands)

        val echob = MoondropDeviceCatalog.classifyUsb("/dev/usb/10", 0x35D8, 0x149B, "Moondrop ECHO-B", true)!!
        assertEquals(DspProtocol.CONEXANT_9B, echob.protocol)

        // Pro must still route to Moondrop 8-band
        val pro = MoondropDeviceCatalog.classifyUsb("/dev/usb/11", 0x35D8, 0xF001, "Moondrop FreeDSP Pro", true)!!
        assertEquals(DspProtocol.MOONDROP_8B, pro.protocol)

        val band = FilterBand(FilterType.LOW_SHELF, 100.0, 3.5, 0.9)
        val cxPkts = MoondropProtocolCodec.conexantBuildBandPackets(1, band)
        assertEquals(5, cxPkts.size)
        assertEquals(62, cxPkts[0].size)
        assertEquals(0x01.toByte(), cxPkts[0][0])
        val bqFlat = MoondropProtocolCodec.conexantComputeBiquad(FilterBand(), 48000.0)
        assertEquals(1073741824, bqFlat[0])

        val eh13 = MoondropDeviceCatalog.classifyBleDevice("FIIO EH13", "AA:BB:CC:DD:EE:01")!!
        assertEquals(DspProtocol.FIIO_BLE, eh13.protocol)
        assertEquals(20.0, eh13.maxGainDb, 0.01)

        val wPkt = MoondropProtocolCodec.fiioBleWriteBandPacket(0, FilterBand(FilterType.LOW_SHELF, 105.0, 4.5, 0.71))
        assertEquals(17, wPkt.size)
        val resp = ByteArray(80)
        resp[0] = 0xF1.toByte()
        resp[1] = 0x10
        resp[3] = 80
        resp[4] = 0x03
        resp[5] = 0x0D
        resp[79] = 0xFF.toByte()
        System.arraycopy(wPkt, 9, resp, 9, 7)
        for (i in 1 until 10) {
            val o = 9 + i * 7
            resp[o + 2] = 0x03
            resp[o + 3] = 0xE8.toByte()
            resp[o + 5] = 100
        }
        val parsedFiio = MoondropProtocolCodec.fiioBleParseEqResponse(resp)!!
        assertEquals(10, parsedFiio.size)
        assertEquals(FilterType.LOW_SHELF, parsedFiio[0].type)
        assertEquals(4.5, parsedFiio[0].gain, 0.01)

        val maxwell = MoondropDeviceCatalog.classifyBleDevice("Audeze Maxwell BT", "AA:BB:CC:DD:EE:02")!!
        assertEquals(DspProtocol.AIROHA_BLE, maxwell.protocol)
        val airohaTx = MoondropProtocolCodec.airohaBleWritePeqPacket(0, listOf(FilterBand(FilterType.HIGH_SHELF, 8000.0, -2.5, 0.75)))
        assertEquals(MoondropProtocolCodec.AIROHA_BLE_RESPONSE_LEN, airohaTx.size)
        val airohaRx = airohaTx.copyOf()
        airohaRx[1] = 0x5B
        val (aEn, aBands) = MoondropProtocolCodec.airohaBleParsePeqResponse(airohaRx)!!
        assertTrue(aEn)
        assertEquals(10, aBands.size)
        assertEquals(FilterType.HIGH_SHELF, aBands[0].type)
        assertEquals(-2.5, aBands[0].gain, 0.02)
    }
}
