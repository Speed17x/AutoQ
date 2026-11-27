package com.speed17x.trivium.widget

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.DashPathEffect
import android.graphics.Paint
import android.graphics.Path
import android.graphics.RectF
import android.graphics.Typeface
import androidx.compose.ui.graphics.toArgb
import com.speed17x.trivium.data.CurvePoint
import com.speed17x.trivium.dsp.BiquadMath
import com.speed17x.trivium.dsp.FilterBand
import com.speed17x.trivium.ui.theme.TriviumThemeTokens
import com.speed17x.trivium.ui.theme.NothingOsTokens
import kotlin.math.abs
import kotlin.math.log10

/**
 * High-DPI Canvas renderers giving the home screen widget modular Trivium theme aesthetics:
 *  - Signature 5x7 dot-matrix pixel display or clean monospace typography
 *  - Dot-field (NothingOS) or line-grid (Default / Default claro) telemetry canvas
 *  - Dynamic theme accent color + live EQ/FR curve overlay
 */
object NothingDotMatrixRenderer {

    private const val PURE_WHITE = 0xFFF5F5F5.toInt()
    private const val MUTED_GRAY = 0xFF7B808A.toInt()

    // 5x7 dot-matrix font glyphs (each string row is 5 columns: '#' = lit dot)
    private val GLYPHS: Map<Char, Array<String>> = mapOf(
        'A' to arrayOf(".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"),
        'B' to arrayOf("####.", "#...#", "#...#", "####.", "#...#", "#...#", "####."),
        'C' to arrayOf(".####", "#....", "#....", "#....", "#....", "#....", ".####"),
        'D' to arrayOf("####.", "#...#", "#...#", "#...#", "#...#", "#...#", "####."),
        'E' to arrayOf("#####", "#....", "#....", "####.", "#....", "#....", "#####"),
        'F' to arrayOf("#####", "#....", "#....", "####.", "#....", "#....", "#...."),
        'G' to arrayOf(".####", "#....", "#....", "#..##", "#...#", "#...#", ".####"),
        'H' to arrayOf("#...#", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"),
        'I' to arrayOf(".###.", "..#..", "..#..", "..#..", "..#..", "..#..", ".###."),
        'J' to arrayOf("..###", "...#.", "...#.", "...#.", "#..#.", "#..#.", ".##.."),
        'K' to arrayOf("#...#", "#..#.", "#.#..", "##...", "#.#..", "#..#.", "#...#"),
        'L' to arrayOf("#....", "#....", "#....", "#....", "#....", "#....", "#####"),
        'M' to arrayOf("#...#", "##.##", "#.#.#", "#...#", "#...#", "#...#", "#...#"),
        'N' to arrayOf("#...#", "##..#", "#.#.#", "#..##", "#...#", "#...#", "#...#"),
        'O' to arrayOf(".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."),
        'P' to arrayOf("####.", "#...#", "#...#", "####.", "#....", "#....", "#...."),
        'Q' to arrayOf(".###.", "#...#", "#...#", "#...#", "#.#.#", "#..#.", ".##.#"),
        'R' to arrayOf("####.", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#"),
        'S' to arrayOf(".####", "#....", "#....", ".###.", "....#", "....#", "####."),
        'T' to arrayOf("#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#.."),
        'U' to arrayOf("#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."),
        'V' to arrayOf("#...#", "#...#", "#...#", "#...#", ".#.#.", ".#.#.", "..#.."),
        'W' to arrayOf("#...#", "#...#", "#...#", "#...#", "#.#.#", "##.##", "#...#"),
        'X' to arrayOf("#...#", "#...#", ".#.#.", "..#..", ".#.#.", "#...#", "#...#"),
        'Y' to arrayOf("#...#", "#...#", ".#.#.", "..#..", "..#..", "..#..", "..#.."),
        'Z' to arrayOf("#####", "....#", "...#.", "..#..", ".#...", "#....", "#####"),
        '0' to arrayOf(".###.", "#..##", "#.#.#", "#.#.#", "##..#", "#...#", ".###."),
        '1' to arrayOf("..#..", ".##..", "..#..", "..#..", "..#..", "..#..", ".###."),
        '2' to arrayOf(".###.", "#...#", "....#", "..##.", ".#...", "#....", "#####"),
        '3' to arrayOf("####.", "....#", "....#", ".###.", "....#", "....#", "####."),
        '4' to arrayOf("...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#."),
        '5' to arrayOf("#####", "#....", "####.", "....#", "....#", "#...#", ".###."),
        '6' to arrayOf(".###.", "#....", "####.", "#...#", "#...#", "#...#", ".###."),
        '7' to arrayOf("#####", "....#", "...#.", "..#..", ".#...", ".#...", ".#..."),
        '8' to arrayOf(".###.", "#...#", "#...#", ".###.", "#...#", "#...#", ".###."),
        '9' to arrayOf(".###.", "#...#", "#...#", ".####", "....#", "....#", ".###."),
        '-' to arrayOf(".....", ".....", ".....", "#####", ".....", ".....", "....."),
        '.' to arrayOf(".....", ".....", ".....", ".....", ".....", ".##..", ".##.."),
        '/' to arrayOf("....#", "...#.", "...#.", "..#..", ".#...", ".#...", "#...."),
        '·' to arrayOf(".....", ".....", ".....", "..#..", ".....", ".....", "....."),
        ' ' to arrayOf(".....", ".....", ".....", ".....", ".....", ".....", "....."),
    )

    fun getGlyph(ch: Char): Array<String> = GLYPHS[ch.uppercaseChar()] ?: GLYPHS[' ']!!

    fun renderDotMatrixTitle(text: String, color: Int = PURE_WHITE): Bitmap {
        val clean = text.uppercase().take(16).ifBlank { "MOONDROP" }
        val dotStep = 5.5f
        val dotRadius = 2.0f
        val charAdvance = 6.4f * dotStep
        val width = (clean.length * charAdvance + dotStep * 2).toInt().coerceAtLeast(180)
        val height = (7 * dotStep + dotStep * 2).toInt()

        val bmp = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bmp)
        val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            this.color = color
            style = Paint.Style.FILL
        }

        clean.forEachIndexed { charIdx, ch ->
            val rows = getGlyph(ch)
            val baseX = dotStep + charIdx * charAdvance
            for (r in 0 until 7) {
                val row = rows[r]
                for (c in 0 until 5) {
                    if (row[c] == '#') {
                        canvas.drawCircle(
                            baseX + c * dotStep,
                            dotStep + r * dotStep,
                            dotRadius,
                            paint,
                        )
                    }
                }
            }
        }
        return bmp
    }

    fun renderCleanTitle(text: String, color: Int): Bitmap {
        val clean = text.uppercase().take(16).ifBlank { "TRIVIUM" }
        val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            this.color = color
            textSize = 36f
            typeface = Typeface.create(Typeface.MONOSPACE, Typeface.BOLD)
            letterSpacing = 0.08f
        }
        val measured = paint.measureText(clean).toInt().coerceAtLeast(180) + 12
        val height = 49
        val bmp = Bitmap.createBitmap(measured, height, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bmp)
        val fm = paint.fontMetrics
        val baseline = (height - fm.ascent - fm.descent) * 0.5f
        canvas.drawText(clean, 2f, baseline, paint)
        return bmp
    }

    fun renderStatusDot(color: Int, sizePx: Int = 24): Bitmap {
        val bmp = Bitmap.createBitmap(sizePx, sizePx, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bmp)
        val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            this.color = color
            style = Paint.Style.FILL
        }
        val r = sizePx * 0.45f
        canvas.drawCircle(sizePx * 0.5f, sizePx * 0.5f, r, paint)
        return bmp
    }

    fun renderPowerPill(fillColor: Int, strokeColor: Int, widthPx: Int = 140, heightPx: Int = 64): Bitmap {
        val bmp = Bitmap.createBitmap(widthPx, heightPx, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bmp)
        val radius = heightPx * 0.5f
        val rect = RectF(1.5f, 1.5f, widthPx - 1.5f, heightPx - 1.5f)
        val fillPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = fillColor
            style = Paint.Style.FILL
        }
        val strokePaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = strokeColor
            strokeWidth = 2.5f
            style = Paint.Style.STROKE
        }
        canvas.drawRoundRect(rect, radius, radius, fillPaint)
        canvas.drawRoundRect(rect, radius, radius, strokePaint)
        return bmp
    }

    fun renderCurveBitmap(
        bands: List<FilterBand>,
        dspEnabled: Boolean,
        overlayFrMode: Boolean,
        stockFr: List<CurvePoint>,
        targetFr: List<CurvePoint>,
        widthPx: Int = 560,
        heightPx: Int = 180,
        tokens: TriviumThemeTokens = NothingOsTokens,
    ): Bitmap {
        val bmp = Bitmap.createBitmap(widthPx, heightPx, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bmp)

        val accentColor = tokens.accent.toArgb()
        val accentGlow = tokens.accent.copy(alpha = 0.27f).toArgb()
        val gridColor = tokens.gridColor.toArgb()
        val zeroColor = tokens.zeroLineColor.toArgb()
        val stockColor = tokens.stockLineColor.toArgb()
        val targetColor = tokens.targetLineColor.copy(alpha = 0.68f).toArgb()
        val textPrimary = tokens.textPrimary.toArgb()
        val textMuted = tokens.textMuted.toArgb()

        val padL = 22f
        val padR = 22f
        val padT = 18f
        val padB = 30f
        val plotW = (widthPx - padL - padR).coerceAtLeast(40f)
        val plotH = (heightPx - padT - padB).coerceAtLeast(40f)

        val useFr = overlayFrMode && stockFr.size >= 8
        val minDb = if (useFr) -14.0 else -12.0
        val maxDb = if (useFr) 14.0 else 12.0
        val logMin = log10(20.0)
        val logMax = log10(20000.0)

        fun freqToX(hz: Double): Float {
            val t = ((log10(hz.coerceIn(20.0, 20000.0)) - logMin) / (logMax - logMin)).toFloat()
            return padL + t * plotW
        }

        fun dbToY(db: Double): Float {
            val t = ((db.coerceIn(minDb, maxDb) - minDb) / (maxDb - minDb)).toFloat()
            return padT + (1f - t) * plotH
        }

        // 1. Theme-appropriate grid: dot matrix for NothingOS, subtle lines for Default / Default claro
        if (tokens.useDotMatrixGrid) {
            val dotPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
                color = gridColor
                style = Paint.Style.FILL
            }
            val dotCols = 22
            val dotRows = 6
            for (r in 0..dotRows) {
                val y = padT + (r.toFloat() / dotRows) * plotH
                for (c in 0..dotCols) {
                    val x = padL + (c.toFloat() / dotCols) * plotW
                    canvas.drawCircle(x, y, 2.0f, dotPaint)
                }
            }
        } else {
            val linePaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
                color = gridColor
                strokeWidth = 1.4f
                style = Paint.Style.STROKE
            }
            for (f in doubleArrayOf(50.0, 100.0, 250.0, 1000.0, 4000.0, 10000.0)) {
                val x = freqToX(f)
                canvas.drawLine(x, padT, x, padT + plotH, linePaint)
            }
            for (db in doubleArrayOf(-6.0, 6.0)) {
                val y = dbToY(db)
                canvas.drawLine(padL, y, padL + plotW, y, linePaint)
            }
        }

        // 2. Subtle 0 dB dotted center reference
        val zeroPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = zeroColor
            strokeWidth = 2.0f
            style = Paint.Style.STROKE
            pathEffect = DashPathEffect(floatArrayOf(5f, 7f), 0f)
        }
        val y0 = dbToY(0.0)
        canvas.drawLine(padL, y0, padL + plotW, y0, zeroPaint)

        // Compute curves on 80-point log grid
        val freqs = BiquadMath.logSpace(20.0, 20000.0, 80)
        val eqResp = if (dspEnabled) BiquadMath.combinedResponseDb(bands, freqs) else DoubleArray(freqs.size)
        val stockInterp = if (useFr) {
            BiquadMath.logInterp(
                freqs,
                stockFr.map { it.freqHz }.toDoubleArray(),
                stockFr.map { it.db }.toDoubleArray(),
            )
        } else {
            DoubleArray(freqs.size)
        }

        // 3. Stock FR dashed line (when FR mode active)
        if (useFr) {
            val stockPath = Path()
            freqs.forEachIndexed { idx, f ->
                val x = freqToX(f)
                val y = dbToY(stockInterp[idx])
                if (idx == 0) stockPath.moveTo(x, y) else stockPath.lineTo(x, y)
            }
            val stockPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
                color = stockColor
                strokeWidth = 2.8f
                style = Paint.Style.STROKE
                pathEffect = DashPathEffect(floatArrayOf(10f, 8f), 0f)
            }
            canvas.drawPath(stockPath, stockPaint)
        }

        // 4. AutoEQ Target dotted curve (when available)
        if (useFr && targetFr.size >= 8) {
            val tgtInterp = BiquadMath.logInterp(
                freqs,
                targetFr.map { it.freqHz }.toDoubleArray(),
                targetFr.map { it.db }.toDoubleArray(),
            )
            val tgtPath = Path()
            freqs.forEachIndexed { idx, f ->
                val x = freqToX(f)
                val y = dbToY(tgtInterp[idx])
                if (idx == 0) tgtPath.moveTo(x, y) else tgtPath.lineTo(x, y)
            }
            val tgtPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
                color = targetColor
                strokeWidth = 2.4f
                style = Paint.Style.STROKE
                pathEffect = DashPathEffect(floatArrayOf(4f, 7f), 0f)
            }
            canvas.drawPath(tgtPath, tgtPaint)
        }

        // 5. Primary Live Curve (Theme accent when ON, muted gray when OFF)
        val liveCurve = DoubleArray(freqs.size) { i ->
            if (useFr) stockInterp[i] + eqResp[i] else eqResp[i]
        }
        val livePath = Path()
        freqs.forEachIndexed { idx, f ->
            val x = freqToX(f)
            val y = dbToY(liveCurve[idx])
            if (idx == 0) livePath.moveTo(x, y) else livePath.lineTo(x, y)
        }

        if (dspEnabled) {
            val glowPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
                color = accentGlow
                strokeWidth = 11f
                style = Paint.Style.STROKE
                strokeCap = Paint.Cap.ROUND
                strokeJoin = Paint.Join.ROUND
            }
            canvas.drawPath(livePath, glowPaint)
        }

        val livePaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = if (dspEnabled) accentColor else textMuted
            strokeWidth = 4.5f
            style = Paint.Style.STROKE
            strokeCap = Paint.Cap.ROUND
            strokeJoin = Paint.Join.ROUND
        }
        canvas.drawPath(livePath, livePaint)

        // 6. Active band node markers
        val nodeFill = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = if (dspEnabled) textPrimary else textMuted
            style = Paint.Style.FILL
        }
        val nodeRing = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = if (dspEnabled) accentColor else zeroColor
            strokeWidth = 3f
            style = Paint.Style.STROKE
        }
        for (b in bands) {
            if (abs(b.gain) < 0.05) continue
            val bx = freqToX(b.frequency)
            val curveVal = BiquadMath.combinedResponseDb(bands, doubleArrayOf(b.frequency))[0]
            val baseVal = if (useFr) {
                BiquadMath.logInterp(
                    doubleArrayOf(b.frequency),
                    stockFr.map { it.freqHz }.toDoubleArray(),
                    stockFr.map { it.db }.toDoubleArray(),
                )[0]
            } else {
                0.0
            }
            val by = dbToY(if (dspEnabled) baseVal + curveVal else baseVal)
            canvas.drawCircle(bx, by, 5.5f, nodeFill)
            canvas.drawCircle(bx, by, 5.5f, nodeRing)
        }

        // 7. Minimal frequency labels at bottom
        val labelPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = textMuted
            textSize = 18f
            typeface = Typeface.MONOSPACE
            textAlign = Paint.Align.CENTER
        }
        val ticks = listOf(30.0 to "30", 100.0 to "100", 500.0 to "500", 2000.0 to "2K", 10000.0 to "10K")
        for ((hz, txt) in ticks) {
            canvas.drawText(txt, freqToX(hz), heightPx - 6f, labelPaint)
        }

        return bmp
    }
}
