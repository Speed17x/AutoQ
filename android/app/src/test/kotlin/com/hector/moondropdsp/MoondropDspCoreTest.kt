package com.hector.moondropdsp

import com.hector.moondropdsp.data.AppPreferencesStore
import com.hector.moondropdsp.data.AutoEqParser
import com.hector.moondropdsp.data.SquigSourceSpec
import com.hector.moondropdsp.dsp.BiquadMath
import com.hector.moondropdsp.dsp.FilterBand
import com.hector.moondropdsp.dsp.FilterType
import com.hector.moondropdsp.hardware.MoondropDeviceCatalog
import com.hector.moondropdsp.hardware.MoondropProtocolCodec
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
}
