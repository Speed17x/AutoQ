package com.hector.moondropdsp.dsp

import kotlinx.serialization.Serializable
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.exp
import kotlin.math.ln
import kotlin.math.log10
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow
import kotlin.math.round
import kotlin.math.roundToInt
import kotlin.math.sin
import kotlin.math.sqrt

const val SAMPLE_RATE_HZ = 48000.0
const val MIN_FREQ_HZ = 20.0
const val MAX_FREQ_HZ = 20000.0
const val DEFAULT_MAX_GAIN_DB = 12.0
const val MIN_Q = 0.1
const val MAX_Q = 10.0

@Serializable
enum class FilterType(val shortCode: String, val apoCode: String, val displayName: String) {
    PEAKING("PK", "PK", "Peaking"),
    LOW_SHELF("LS", "LSC", "Low Shelf"),
    HIGH_SHELF("HS", "HSC", "High Shelf"),
}

@Serializable
data class FilterBand(
    val type: FilterType = FilterType.PEAKING,
    val frequency: Double = 1000.0,
    val gain: Double = 0.0,
    val q: Double = 1.0,
) {
    fun clamped(maxGainDb: Double = DEFAULT_MAX_GAIN_DB, allowShelves: Boolean = true): FilterBand {
        val safeType = if (!allowShelves && type != FilterType.PEAKING) FilterType.PEAKING else type
        val safeFreq = round(frequency.coerceIn(MIN_FREQ_HZ, MAX_FREQ_HZ))
        val safeGain = round(gain.coerceIn(-maxGainDb, maxGainDb) * 10.0) / 10.0
        val safeQ = round(q.coerceIn(MIN_Q, MAX_Q) * 1000.0) / 1000.0
        return FilterBand(type = safeType, frequency = safeFreq, gain = safeGain, q = safeQ)
    }
}

data class BiquadCoeffs(
    val b0: Double,
    val b1: Double,
    val b2: Double,
    val a0: Double,
    val a1: Double,
    val a2: Double,
)

object BiquadMath {

    const val DEFAULT_VIRTUAL_BANDS = 10

    val HARMAN_WARM_BANDS: List<FilterBand> = listOf(
        FilterBand(FilterType.LOW_SHELF, 105.0, 3.2, 0.707),
        FilterBand(FilterType.PEAKING, 240.0, -1.4, 1.100),
        FilterBand(FilterType.PEAKING, 1000.0, 0.0, 1.000),
        FilterBand(FilterType.PEAKING, 2800.0, 1.8, 1.400),
        FilterBand(FilterType.PEAKING, 6200.0, -1.6, 3.200),
        FilterBand(FilterType.HIGH_SHELF, 9500.0, 1.2, 0.707),
    )

    val VOCAL_REF_BANDS: List<FilterBand> = listOf(
        FilterBand(FilterType.LOW_SHELF, 85.0, -1.5, 0.707),
        FilterBand(FilterType.PEAKING, 320.0, -1.0, 1.200),
        FilterBand(FilterType.PEAKING, 1600.0, 2.2, 1.300),
        FilterBand(FilterType.PEAKING, 3600.0, 1.5, 1.800),
        FilterBand(FilterType.HIGH_SHELF, 10000.0, 0.8, 0.707),
    )

    val BASS_BOOST_BANDS: List<FilterBand> = listOf(
        FilterBand(FilterType.LOW_SHELF, 90.0, 4.5, 0.707),
        FilterBand(FilterType.PEAKING, 220.0, -1.0, 1.200),
        FilterBand(FilterType.PEAKING, 1000.0, 0.0, 1.000),
        FilterBand(FilterType.PEAKING, 3200.0, 0.8, 1.400),
        FilterBand(FilterType.HIGH_SHELF, 10000.0, 0.5, 0.707),
    )

    fun flatBands(count: Int = DEFAULT_VIRTUAL_BANDS): List<FilterBand> {
        val defaultFreqs = if (count <= 5) {
            doubleArrayOf(60.0, 250.0, 1000.0, 3500.0, 8000.0)
        } else {
            doubleArrayOf(31.0, 63.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0)
        }
        return List(count.coerceAtLeast(1)) { i ->
            FilterBand(FilterType.PEAKING, defaultFreqs[i % defaultFreqs.size], 0.0, 1.0)
        }
    }

    fun logSpace(minHz: Double = MIN_FREQ_HZ, maxHz: Double = MAX_FREQ_HZ, count: Int = 96): DoubleArray {
        val n = count.coerceAtLeast(2)
        val lo = log10(minHz)
        val hi = log10(maxHz)
        return DoubleArray(n) { i ->
            10.0.pow(lo + (hi - lo) * i / (n - 1).toDouble())
        }
    }

    fun logInterp(targetFreqs: DoubleArray, srcFreqs: DoubleArray, srcValues: DoubleArray): DoubleArray {
        if (srcFreqs.isEmpty() || srcValues.isEmpty()) return DoubleArray(targetFreqs.size)
        val logSrc = DoubleArray(srcFreqs.size) { log10(srcFreqs[it].coerceAtLeast(1.0)) }
        return DoubleArray(targetFreqs.size) { idx ->
            val x = log10(targetFreqs[idx].coerceAtLeast(1.0))
            when {
                x <= logSrc.first() -> srcValues.first()
                x >= logSrc.last() -> srcValues.last()
                else -> {
                    var lo = 0
                    var hi = logSrc.lastIndex
                    while (hi - lo > 1) {
                        val mid = (lo + hi) ushr 1
                        if (logSrc[mid] <= x) lo = mid else hi = mid
                    }
                    val span = (logSrc[hi] - logSrc[lo]).coerceAtLeast(1e-12)
                    val t = (x - logSrc[lo]) / span
                    srcValues[lo] + (srcValues[hi] - srcValues[lo]) * t
                }
            }
        }
    }

    fun biquadCoeffs(band: FilterBand, fs: Double = SAMPLE_RATE_HZ): BiquadCoeffs {
        val f0 = band.frequency.coerceIn(1.0, fs * 0.495)
        val q = band.q.coerceIn(MIN_Q, MAX_Q)
        val gainDb = band.gain.coerceIn(-DEFAULT_MAX_GAIN_DB, DEFAULT_MAX_GAIN_DB)

        if (abs(gainDb) < 1e-6) {
            return BiquadCoeffs(1.0, 0.0, 0.0, 1.0, 0.0, 0.0)
        }

        val a = 10.0.pow(gainDb / 40.0)
        val w0 = 2.0 * PI * f0 / fs
        val cosW0 = cos(w0)
        val sinW0 = sin(w0)
        val alpha = sinW0 / (2.0 * q)

        return when (band.type) {
            FilterType.PEAKING -> BiquadCoeffs(
                b0 = 1.0 + alpha * a,
                b1 = -2.0 * cosW0,
                b2 = 1.0 - alpha * a,
                a0 = 1.0 + alpha / a,
                a1 = -2.0 * cosW0,
                a2 = 1.0 - alpha / a,
            )

            FilterType.LOW_SHELF -> {
                val twoSqrtAAlpha = 2.0 * sqrt(a) * alpha
                BiquadCoeffs(
                    b0 = a * ((a + 1.0) - (a - 1.0) * cosW0 + twoSqrtAAlpha),
                    b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cosW0),
                    b2 = a * ((a + 1.0) - (a - 1.0) * cosW0 - twoSqrtAAlpha),
                    a0 = (a + 1.0) + (a - 1.0) * cosW0 + twoSqrtAAlpha,
                    a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cosW0),
                    a2 = (a + 1.0) + (a - 1.0) * cosW0 - twoSqrtAAlpha,
                )
            }

            FilterType.HIGH_SHELF -> {
                val twoSqrtAAlpha = 2.0 * sqrt(a) * alpha
                BiquadCoeffs(
                    b0 = a * ((a + 1.0) + (a - 1.0) * cosW0 + twoSqrtAAlpha),
                    b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cosW0),
                    b2 = a * ((a + 1.0) + (a - 1.0) * cosW0 - twoSqrtAAlpha),
                    a0 = (a + 1.0) - (a - 1.0) * cosW0 + twoSqrtAAlpha,
                    a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cosW0),
                    a2 = (a + 1.0) - (a - 1.0) * cosW0 - twoSqrtAAlpha,
                )
            }
        }
    }

    /** Exact Moondrop 8-band 96 kHz Q30 biquad encoding matching chu2_hid.py. */
    fun encodeMd8BiquadQ30(band: FilterBand): IntArray {
        val b = band.clamped()
        val a = 10.0.pow(b.gain / 40.0)
        val w0 = 2.0 * PI * b.frequency / 96000.0
        val sinW0 = sin(w0)
        val cosW0 = cos(w0)
        val alpha = sinW0 / (2.0 * b.q)

        val b0: Double
        val b1: Double
        val b2: Double
        val a0: Double
        val a1: Double
        val a2: Double
        if (b.type == FilterType.LOW_SHELF || b.type == FilterType.HIGH_SHELF) {
            val shelfAlpha = (sinW0 / 2.0) * sqrt(max(1e-9, (a + 1.0 / a) * (1.0 / b.q - 1.0) + 2.0))
            val s = 2.0 * sqrt(a) * shelfAlpha
            if (b.type == FilterType.LOW_SHELF) {
                b0 = a * ((a + 1.0) - (a - 1.0) * cosW0 + s)
                b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cosW0)
                b2 = a * ((a + 1.0) - (a - 1.0) * cosW0 - s)
                a0 = (a + 1.0) + (a - 1.0) * cosW0 + s
                a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cosW0)
                a2 = (a + 1.0) + (a - 1.0) * cosW0 - s
            } else {
                b0 = a * ((a + 1.0) + (a - 1.0) * cosW0 + s)
                b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cosW0)
                b2 = a * ((a + 1.0) + (a - 1.0) * cosW0 - s)
                a0 = (a + 1.0) - (a - 1.0) * cosW0 + s
                a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cosW0)
                a2 = (a + 1.0) - (a - 1.0) * cosW0 - s
            }
        } else {
            b0 = 1.0 + alpha * a
            b1 = -2.0 * cosW0
            b2 = 1.0 - alpha * a
            a0 = 1.0 + alpha / a
            a1 = -2.0 * cosW0
            a2 = 1.0 - alpha / a
        }

        val scale = 1073741824.0 // 2^30
        return intArrayOf(
            round((b0 / a0) * scale).toInt(),
            round((b1 / a0) * scale).toInt(),
            round((b2 / a0) * scale).toInt(),
            round(-(a1 / a0) * scale).toInt(),
            round(-(a2 / a0) * scale).toInt(),
        )
    }

    fun bandResponseDb(band: FilterBand, freqs: DoubleArray, fs: Double = SAMPLE_RATE_HZ): DoubleArray {
        val out = DoubleArray(freqs.size)
        if (abs(band.gain) < 1e-6) return out

        val c = biquadCoeffs(band, fs)
        val b0n = c.b0 / c.a0
        val b1n = c.b1 / c.a0
        val b2n = c.b2 / c.a0
        val a1n = c.a1 / c.a0
        val a2n = c.a2 / c.a0

        val numConst = (b0n + b1n + b2n).let { it * it }
        val numLin = 4.0 * (b0n * b1n + 4.0 * b0n * b2n + b1n * b2n)
        val numQuad = 16.0 * b0n * b2n

        val denConst = (1.0 + a1n + a2n).let { it * it }
        val denLin = 4.0 * (a1n + 4.0 * a2n + a1n * a2n)
        val denQuad = 16.0 * a2n

        for (i in freqs.indices) {
            val w = 2.0 * PI * freqs[i] / fs
            val s = sin(w * 0.5)
            val phi = s * s
            val num = max(1e-18, numConst - numLin * phi + numQuad * phi * phi)
            val den = max(1e-18, denConst - denLin * phi + denQuad * phi * phi)
            out[i] = 10.0 * log10(num / den)
        }
        return out
    }

    fun combinedResponseDb(bands: List<FilterBand>, freqs: DoubleArray, fs: Double = SAMPLE_RATE_HZ): DoubleArray {
        val total = DoubleArray(freqs.size)
        for (band in bands) {
            if (abs(band.gain) < 1e-6) continue
            val r = bandResponseDb(band, freqs, fs)
            for (i in freqs.indices) total[i] += r[i]
        }
        return total
    }

    fun peakBoostDb(bands: List<FilterBand>): Double {
        if (bands.isEmpty()) return 0.0
        val freqs = logSpace(MIN_FREQ_HZ, MAX_FREQ_HZ, 256)
        val resp = combinedResponseDb(bands, freqs)
        val peak = resp.maxOrNull() ?: 0.0
        return if (peak > 0.0) round(peak * 100.0) / 100.0 else 0.0
    }

    fun computeSafePreampDb(bands: List<FilterBand>): Double {
        val peak = peakBoostDb(bands)
        return if (peak > 0.0) round((-peak - 0.2).coerceIn(-18.0, 0.0) * 100.0) / 100.0 else 0.0
    }

    fun headroomMarginDb(bands: List<FilterBand>, preampDb: Double): Double {
        return round(-(peakBoostDb(bands) + preampDb) * 100.0) / 100.0
    }

    fun makeWeights(freqs: DoubleArray, maxFreqFit: Double): DoubleArray {
        val ln2 = ln(2.0)
        return DoubleArray(freqs.size) { i ->
            val f = freqs[i]
            when {
                f < 28.0 -> 0.65
                f > maxFreqFit -> {
                    val octOver = ln(f / maxFreqFit) / ln2
                    max(0.002, exp(-5.0 * octOver))
                }
                else -> 1.0
            }
        }
    }

    fun weightedRmse(err: DoubleArray, weights: DoubleArray): Double {
        var num = 0.0
        var den = 0.0
        for (i in err.indices) {
            num += weights[i] * err[i] * err[i]
            den += weights[i]
        }
        return if (den <= 0.0) 0.0 else sqrt(num / den)
    }

    private fun weightedLoss(target: DoubleArray, curve: DoubleArray, weights: DoubleArray): Double {
        var acc = 0.0
        for (i in target.indices) {
            val d = target[i] - curve[i]
            acc += weights[i] * d * d
        }
        return acc
    }

    private fun estimateInitialQ(freqs: DoubleArray, residual: DoubleArray, peakIdx: Int): Double {
        val peakVal = residual[peakIdx]
        if (abs(peakVal) < 1e-3) return 1.0
        val sign = if (peakVal > 0.0) 1.0 else -1.0
        val halfVal = abs(peakVal) * 0.5

        var left = peakIdx
        while (left > 0 && residual[left] * sign > halfVal) left--
        var right = peakIdx
        while (right < freqs.lastIndex && residual[right] * sign > halfVal) right++

        val fLo = freqs[left]
        val fHi = freqs[right]
        if (fHi <= fLo) return 2.0
        val bwOct = ln(fHi / fLo) / ln(2.0)
        return (1.4 / max(bwOct, 0.14)).coerceIn(0.35, 6.0)
    }

    /**
     * Two-stage PEQ optimizer matching biquad.py:
     * Stage 1: Greedy residual peak/shelf selection.
     * Stage 2: Joint coordinate descent over (log-freq, gain, log-Q).
     */
    fun fitPeqBands(
        freqs: DoubleArray,
        targetDb: DoubleArray,
        maxBands: Int = 5,
        maxFreqFit: Double = 10000.0,
        maxGainDb: Double = DEFAULT_MAX_GAIN_DB,
        supportsShelves: Boolean = true,
    ): Pair<List<FilterBand>, Double> {
        val weights = makeWeights(freqs, maxFreqFit)
        val sqrtWeights = DoubleArray(weights.size) { sqrt(weights[it]) }
        val bands = mutableListOf<FilterBand>()
        val bandCurves = mutableListOf<DoubleArray>()
        val current = DoubleArray(freqs.size)

        // Stage 1: Greedy residual matching
        for (step in 0 until maxBands) {
            val residual = DoubleArray(freqs.size) { targetDb[it] - current[it] }
            var bestIdx = 0
            var bestScore = -1.0
            for (i in freqs.indices) {
                val sc = abs(residual[i]) * sqrtWeights[i]
                if (sc > bestScore) {
                    bestScore = sc
                    bestIdx = i
                }
            }
            if (abs(residual[bestIdx]) < 0.25) break

            val f0 = freqs[bestIdx].coerceIn(MIN_FREQ_HZ, MAX_FREQ_HZ)
            val g0 = residual[bestIdx].coerceIn(-maxGainDb, maxGainDb)
            val qEst = round(estimateInitialQ(freqs, residual, bestIdx) * 100.0) / 100.0

            val candidates = mutableListOf<FilterBand>()
            for (qTry in setOf(qEst, 0.707, 1.4, 2.8, 4.5)) {
                candidates += FilterBand(FilterType.PEAKING, f0, g0, qTry)
            }
            if (supportsShelves && f0 <= 250.0) {
                for (fLs in doubleArrayOf(60.0, 105.0, 160.0)) {
                    candidates += FilterBand(FilterType.LOW_SHELF, fLs, g0 * 0.85, 0.707)
                }
            }
            if (supportsShelves && f0 >= 4500.0) {
                for (fHs in doubleArrayOf(6000.0, 8500.0, 10500.0)) {
                    candidates += FilterBand(FilterType.HIGH_SHELF, fHs, g0 * 0.85, 0.707)
                }
            }

            var bestBand = candidates.first()
            var bestCurve = bandResponseDb(bestBand, freqs)
            var bestLoss = weightedLoss(residual, bestCurve, weights)
            for (i in 1 until candidates.size) {
                val cand = candidates[i]
                val curve = bandResponseDb(cand, freqs)
                val loss = weightedLoss(residual, curve, weights)
                if (loss < bestLoss) {
                    bestLoss = loss
                    bestBand = cand
                    bestCurve = curve
                }
            }

            bands += bestBand
            bandCurves += bestCurve
            for (i in freqs.indices) current[i] += bestCurve[i]
        }

        // Stage 2: Joint coordinate-descent refinement
        if (bands.isNotEmpty()) {
            val schedules = arrayOf(
                doubleArrayOf(0.18, 2.0, 0.22),
                doubleArrayOf(0.09, 1.0, 0.12),
                doubleArrayOf(0.04, 0.4, 0.06),
                doubleArrayOf(0.015, 0.15, 0.025),
                doubleArrayOf(0.006, 0.1, 0.01),
            )
            val others = DoubleArray(freqs.size)
            val resI = DoubleArray(freqs.size)

            for (sched in schedules) {
                val dfOct = sched[0]
                val dgDb = sched[1]
                val dqOct = sched[2]
                repeat(2) {
                    for (i in bands.indices) {
                        val curCurve = bandCurves[i]
                        for (k in freqs.indices) {
                            val o = current[k] - curCurve[k]
                            others[k] = o
                            resI[k] = targetDb[k] - o
                        }
                        var b = bands[i]
                        var bestCurve = curCurve
                        var bestLoss = weightedLoss(resI, bestCurve, weights)

                        for (delta in doubleArrayOf(-dfOct, dfOct)) {
                            val nf = b.frequency * 2.0.pow(delta)
                            if (nf in MIN_FREQ_HZ..MAX_FREQ_HZ) {
                                val nb = b.copy(frequency = nf)
                                val nc = bandResponseDb(nb, freqs)
                                val loss = weightedLoss(resI, nc, weights)
                                if (loss < bestLoss) {
                                    bestLoss = loss
                                    b = nb
                                    bestCurve = nc
                                }
                            }
                        }

                        for (delta in doubleArrayOf(-dgDb, dgDb)) {
                            val ng = (b.gain + delta).coerceIn(-maxGainDb, maxGainDb)
                            if (ng != b.gain) {
                                val nb = b.copy(gain = ng)
                                val nc = bandResponseDb(nb, freqs)
                                val loss = weightedLoss(resI, nc, weights)
                                if (loss < bestLoss) {
                                    bestLoss = loss
                                    b = nb
                                    bestCurve = nc
                                }
                            }
                        }

                        for (delta in doubleArrayOf(-dqOct, dqOct)) {
                            val nq = (b.q * 2.0.pow(delta)).coerceIn(MIN_Q, MAX_Q)
                            if (nq != b.q) {
                                val nb = b.copy(q = nq)
                                val nc = bandResponseDb(nb, freqs)
                                val loss = weightedLoss(resI, nc, weights)
                                if (loss < bestLoss) {
                                    bestLoss = loss
                                    b = nb
                                    bestCurve = nc
                                }
                            }
                        }

                        bands[i] = b
                        bandCurves[i] = bestCurve
                        for (k in freqs.indices) current[k] = others[k] + bestCurve[k]
                    }
                }
            }
        }

        // Quantize to hardware resolution, sort ascending by frequency, and pad
        val quantized = bands
            .map { it.clamped(maxGainDb = maxGainDb, allowShelves = supportsShelves) }
            .filter { abs(it.gain) >= 0.1 }
            .sortedBy { it.frequency }
            .toMutableList()

        while (quantized.size < maxBands) {
            quantized += FilterBand(FilterType.PEAKING, 1000.0, 0.0, 1.0)
        }
        val finalBands = quantized.take(maxBands)
        val finalResp = combinedResponseDb(finalBands, freqs)
        val err = DoubleArray(freqs.size) { targetDb[it] - finalResp[it] }
        return finalBands to weightedRmse(err, weights)
    }

    fun formatFreq(hz: Double): String {
        val r = hz.roundToInt()
        return if (r >= 1000) {
            val k = r / 1000.0
            if (r % 1000 == 0) "${r / 1000}k" else String.format(java.util.Locale.US, "%.1fk", k)
        } else {
            "${r}Hz"
        }
    }
}
