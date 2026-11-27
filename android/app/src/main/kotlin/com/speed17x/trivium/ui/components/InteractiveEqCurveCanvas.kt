package com.speed17x.trivium.ui.components

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.unit.dp
import com.speed17x.trivium.data.CurvePoint
import com.speed17x.trivium.dsp.BiquadMath
import com.speed17x.trivium.dsp.FilterBand
import com.speed17x.trivium.ui.theme.LocalTriviumTheme
import kotlin.math.hypot
import kotlin.math.log10
import kotlin.math.pow
import kotlin.math.round

@Composable
fun InteractiveEqCurveCanvas(
    bands: List<FilterBand>,
    selectedBandIdx: Int,
    dspEnabled: Boolean,
    overlayFrMode: Boolean,
    maxGainDb: Double,
    stockFr: List<CurvePoint>,
    targetFr: List<CurvePoint>,
    targetLabel: String,
    onSelectBand: (Int) -> Unit,
    onDragBand: (index: Int, newFreqHz: Double, newGainDb: Double) -> Unit,
    onDragEnd: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val useFr = overlayFrMode && stockFr.size >= 8
    val minDb = if (useFr) -15.0 else -12.0
    val maxDb = if (useFr) 15.0 else 12.0
    val logMin = remember { log10(20.0) }
    val logMax = remember { log10(20000.0) }
    val freqs = remember { BiquadMath.logSpace(20.0, 20000.0, 96) }

    // Precompute interpolated curves & source arrays once per curve change (zero allocations on 60/120Hz redraws)
    val stockArrays = remember(stockFr) {
        if (stockFr.size >= 8) {
            stockFr.map { it.freqHz }.toDoubleArray() to stockFr.map { it.db }.toDoubleArray()
        } else {
            null
        }
    }
    val stockInterp = remember(stockArrays, useFr) {
        if (useFr && stockArrays != null) {
            BiquadMath.logInterp(freqs, stockArrays.first, stockArrays.second)
        } else {
            DoubleArray(freqs.size)
        }
    }
    val targetInterp = remember(targetFr, useFr) {
        if (useFr && targetFr.size >= 8) {
            BiquadMath.logInterp(
                freqs,
                targetFr.map { it.freqHz }.toDoubleArray(),
                targetFr.map { it.db }.toDoubleArray(),
            )
        } else {
            null
        }
    }
    val eqResp = remember(bands, dspEnabled) {
        if (dspEnabled) BiquadMath.combinedResponseDb(bands, freqs) else DoubleArray(freqs.size)
    }
    val selectedBandResp = remember(bands, selectedBandIdx, dspEnabled) {
        if (dspEnabled && selectedBandIdx in bands.indices) {
            BiquadMath.bandResponseDb(bands[selectedBandIdx], freqs)
        } else {
            null
        }
    }
    val bandBaseOffsets = remember(bands, stockArrays, useFr) {
        if (useFr && stockArrays != null && bands.isNotEmpty()) {
            BiquadMath.logInterp(
                DoubleArray(bands.size) { bands[it].frequency },
                stockArrays.first,
                stockArrays.second,
            )
        } else {
            DoubleArray(bands.size)
        }
    }

    // Hoist Path and PathEffect objects so onDraw creates zero GC churn
    val stockPath = remember { Path() }
    val tgtPath = remember { Path() }
    val selPath = remember { Path() }
    val livePath = remember { Path() }
    val dashZero = remember { PathEffect.dashPathEffect(floatArrayOf(5f, 7f), 0f) }
    val dashStock = remember { PathEffect.dashPathEffect(floatArrayOf(10f, 8f), 0f) }
    val dashTarget = remember { PathEffect.dashPathEffect(floatArrayOf(4f, 6f), 0f) }

    val currentBands by rememberUpdatedState(bands)
    val currentUseFr by rememberUpdatedState(useFr)
    val currentStockArrays by rememberUpdatedState(stockArrays)
    val currentSelected by rememberUpdatedState(selectedBandIdx)
    val currentMaxGain by rememberUpdatedState(maxGainDb)
    val currentOnSelect by rememberUpdatedState(onSelectBand)
    val currentOnDrag by rememberUpdatedState(onDragBand)
    val currentOnDragEnd by rememberUpdatedState(onDragEnd)

    var draggingIdx by remember { mutableIntStateOf(-1) }
    val theme = LocalTriviumTheme.current

    Column(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(theme.cardRadiusDp.dp))
            .background(theme.graphBg)
            .border(1.dp, theme.border, RoundedCornerShape(theme.cardRadiusDp.dp))
            .padding(horizontal = 10.dp, vertical = 10.dp),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 4.dp, vertical = 2.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(
                    text = if (dspEnabled) "● LIVE" else "○ BYPASS",
                    color = if (dspEnabled) theme.accent else theme.textMuted,
                    style = MaterialTheme.typography.labelSmall,
                )
                if (useFr) {
                    Text(
                        text = "╌ STOCK",
                        color = theme.textMuted,
                        style = MaterialTheme.typography.labelSmall,
                    )
                }
                if (useFr && targetInterp != null) {
                    Text(
                        text = "┄ ${targetLabel.uppercase().take(16)}",
                        color = theme.textPrimary.copy(alpha = 0.78f),
                        style = MaterialTheme.typography.labelSmall,
                    )
                }
            }
            Text(
                text = if (useFr) "±15dB · FR" else "±12dB · ΔEQ",
                color = theme.textSubtle,
                style = MaterialTheme.typography.labelSmall,
            )
        }

        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(190.dp)
                .padding(top = 4.dp),
        ) {
            Canvas(
                modifier = Modifier
                    .fillMaxSize()
                    .pointerInput(Unit) {
                        val padL = 10f
                        val padR = 10f
                        val padT = 14f
                        val padB = 14f
                        val singleFreq = DoubleArray(1)

                        fun freqToX(hz: Double, w: Float): Float {
                            val pw = (w - padL - padR).coerceAtLeast(10f)
                            val t = ((log10(hz.coerceIn(20.0, 20000.0)) - logMin) / (logMax - logMin)).toFloat()
                            return padL + t * pw
                        }

                        fun dbToY(db: Double, h: Float): Float {
                            val ph = (h - padT - padB).coerceAtLeast(10f)
                            val mn = if (currentUseFr) -15.0 else -12.0
                            val mx = if (currentUseFr) 15.0 else 12.0
                            val t = ((db.coerceIn(mn, mx) - mn) / (mx - mn)).toFloat()
                            return padT + (1f - t) * ph
                        }

                        fun stockAt(freq: Double): Double {
                            val arr = currentStockArrays
                            if (!currentUseFr || arr == null) return 0.0
                            singleFreq[0] = freq
                            return BiquadMath.logInterp(singleFreq, arr.first, arr.second)[0]
                        }

                        detectTapGestures { pos ->
                            val w = size.width.toFloat()
                            val h = size.height.toFloat()
                            var best = -1
                            var bestDist = 72f
                            currentBands.forEachIndexed { idx, b ->
                                val bx = freqToX(b.frequency, w)
                                val by = dbToY(stockAt(b.frequency) + b.gain, h)
                                val d = hypot(pos.x - bx, pos.y - by)
                                if (d < bestDist) {
                                    bestDist = d
                                    best = idx
                                }
                            }
                            if (best >= 0) currentOnSelect(best)
                        }
                    }
                    .pointerInput(Unit) {
                        val padL = 10f
                        val padR = 10f
                        val padT = 14f
                        val padB = 14f
                        val singleFreq = DoubleArray(1)

                        fun freqToX(hz: Double, w: Float): Float {
                            val pw = (w - padL - padR).coerceAtLeast(10f)
                            val t = ((log10(hz.coerceIn(20.0, 20000.0)) - logMin) / (logMax - logMin)).toFloat()
                            return padL + t * pw
                        }

                        fun xToFreq(x: Float, w: Float): Double {
                            val pw = (w - padL - padR).coerceAtLeast(10f)
                            val t = ((x - padL) / pw).coerceIn(0f, 1f).toDouble()
                            return round(10.0.pow(logMin + t * (logMax - logMin))).coerceIn(20.0, 20000.0)
                        }

                        fun dbToY(db: Double, h: Float): Float {
                            val ph = (h - padT - padB).coerceAtLeast(10f)
                            val mn = if (currentUseFr) -15.0 else -12.0
                            val mx = if (currentUseFr) 15.0 else 12.0
                            val t = ((db.coerceIn(mn, mx) - mn) / (mx - mn)).toFloat()
                            return padT + (1f - t) * ph
                        }

                        fun yToDb(y: Float, h: Float): Double {
                            val ph = (h - padT - padB).coerceAtLeast(10f)
                            val mn = if (currentUseFr) -15.0 else -12.0
                            val mx = if (currentUseFr) 15.0 else 12.0
                            val t = (1f - ((y - padT) / ph)).coerceIn(0f, 1f).toDouble()
                            return mn + t * (mx - mn)
                        }

                        fun stockAt(freq: Double): Double {
                            val arr = currentStockArrays
                            if (!currentUseFr || arr == null) return 0.0
                            singleFreq[0] = freq
                            return BiquadMath.logInterp(singleFreq, arr.first, arr.second)[0]
                        }

                        detectDragGestures(
                            onDragStart = { start ->
                                val w = size.width.toFloat()
                                val h = size.height.toFloat()
                                var best = -1
                                var bestDist = 78f
                                currentBands.forEachIndexed { idx, b ->
                                    val bx = freqToX(b.frequency, w)
                                    val by = dbToY(stockAt(b.frequency) + b.gain, h)
                                    val d = hypot(start.x - bx, start.y - by)
                                    if (d < bestDist) {
                                        bestDist = d
                                        best = idx
                                    }
                                }
                                draggingIdx = if (best >= 0) best else currentSelected
                                if (draggingIdx in currentBands.indices) {
                                    currentOnSelect(draggingIdx)
                                }
                            },
                            onDragEnd = {
                                draggingIdx = -1
                                currentOnDragEnd()
                            },
                            onDragCancel = {
                                draggingIdx = -1
                                currentOnDragEnd()
                            },
                            onDrag = { change, _ ->
                                change.consume()
                                val idx = draggingIdx
                                if (idx !in currentBands.indices) return@detectDragGestures
                                val newFreq = xToFreq(change.position.x, size.width.toFloat())
                                val rawDb = yToDb(change.position.y, size.height.toFloat())
                                val gain = (rawDb - stockAt(newFreq)).coerceIn(-currentMaxGain, currentMaxGain)
                                val roundedGain = round(gain * 10.0) / 10.0
                                currentOnDrag(idx, newFreq, roundedGain)
                            },
                        )
                    },
            ) {
                val padL = 10f
                val padR = 10f
                val padT = 14f
                val padB = 14f
                val pw = (size.width - padL - padR).coerceAtLeast(10f)
                val ph = (size.height - padT - padB).coerceAtLeast(10f)

                fun freqToX(hz: Double): Float {
                    val t = ((log10(hz.coerceIn(20.0, 20000.0)) - logMin) / (logMax - logMin)).toFloat()
                    return padL + t * pw
                }

                fun dbToY(db: Double): Float {
                    val t = ((db.coerceIn(minDb, maxDb) - minDb) / (maxDb - minDb)).toFloat()
                    return padT + (1f - t) * ph
                }

                // 1. Background Grid (dot-matrix for Nothing OS, clean lines for Android/HyperOS)
                if (theme.useDotMatrixGrid) {
                    val cols = 20
                    val rows = 6
                    for (r in 0..rows) {
                        val y = padT + (r.toFloat() / rows) * ph
                        for (c in 0..cols) {
                            val x = padL + (c.toFloat() / cols) * pw
                            drawCircle(
                                color = theme.gridColor,
                                radius = 1.6f,
                                center = Offset(x, y),
                            )
                        }
                    }
                } else {
                    val rows = 6
                    for (r in 0..rows) {
                        val y = padT + (r.toFloat() / rows) * ph
                        drawLine(
                            color = theme.gridColor,
                            start = Offset(padL, y),
                            end = Offset(padL + pw, y),
                            strokeWidth = 1.0f,
                        )
                    }
                    val cols = 6
                    for (c in 0..cols) {
                        val x = padL + (c.toFloat() / cols) * pw
                        drawLine(
                            color = theme.gridColor,
                            start = Offset(x, padT),
                            end = Offset(x, padT + ph),
                            strokeWidth = 1.0f,
                        )
                    }
                }

                // 2. Center 0 dB hairline
                val y0 = dbToY(0.0)
                drawLine(
                    color = theme.zeroLineColor,
                    start = Offset(padL, y0),
                    end = Offset(padL + pw, y0),
                    strokeWidth = 1.4f,
                    pathEffect = if (theme.useDotMatrixGrid) dashZero else null,
                )

                // 3. Stock FR curve
                if (useFr) {
                    stockPath.reset()
                    for (i in freqs.indices) {
                        val x = freqToX(freqs[i])
                        val y = dbToY(stockInterp[i])
                        if (i == 0) stockPath.moveTo(x, y) else stockPath.lineTo(x, y)
                    }
                    drawPath(
                        path = stockPath,
                        color = theme.stockLineColor,
                        style = Stroke(width = 2.2f, pathEffect = dashStock),
                    )
                }

                // 4. Target FR curve
                if (useFr && targetInterp != null) {
                    tgtPath.reset()
                    for (i in freqs.indices) {
                        val x = freqToX(freqs[i])
                        val y = dbToY(targetInterp[i])
                        if (i == 0) tgtPath.moveTo(x, y) else tgtPath.lineTo(x, y)
                    }
                    drawPath(
                        path = tgtPath,
                        color = theme.targetLineColor,
                        style = Stroke(width = 2.0f, pathEffect = dashTarget),
                    )
                }

                // 5. Selected individual band preview curve
                if (selectedBandResp != null) {
                    selPath.reset()
                    for (i in freqs.indices) {
                        val v = if (useFr) stockInterp[i] + selectedBandResp[i] else selectedBandResp[i]
                        val x = freqToX(freqs[i])
                        val y = dbToY(v)
                        if (i == 0) selPath.moveTo(x, y) else selPath.lineTo(x, y)
                    }
                    drawPath(
                        path = selPath,
                        color = theme.accent.copy(alpha = 0.28f),
                        style = Stroke(width = 2.0f),
                    )
                }

                // 6. Primary Live Curve
                livePath.reset()
                for (i in freqs.indices) {
                    val v = if (useFr) stockInterp[i] + eqResp[i] else eqResp[i]
                    val x = freqToX(freqs[i])
                    val y = dbToY(v)
                    if (i == 0) livePath.moveTo(x, y) else livePath.lineTo(x, y)
                }
                if (dspEnabled) {
                    drawPath(
                        path = livePath,
                        color = theme.accent.copy(alpha = 0.25f),
                        style = Stroke(width = 9f, cap = StrokeCap.Round, join = StrokeJoin.Round),
                    )
                }
                drawPath(
                    path = livePath,
                    color = if (dspEnabled) theme.accent else theme.textMuted,
                    style = Stroke(width = 3.6f, cap = StrokeCap.Round, join = StrokeJoin.Round),
                )

                // 7. Interactive band handles
                for (idx in bands.indices) {
                    val b = bands[idx]
                    val bx = freqToX(b.frequency)
                    val base = if (idx < bandBaseOffsets.size) bandBaseOffsets[idx] else 0.0
                    val by = dbToY(base + b.gain)
                    val isSel = idx == selectedBandIdx

                    if (isSel) {
                        drawCircle(
                            color = theme.accent.copy(alpha = 0.22f),
                            radius = 18f,
                            center = Offset(bx, by),
                        )
                    }
                    drawCircle(
                        color = if (isSel) theme.accent else theme.graphBg,
                        radius = if (isSel) 9.5f else 6.0f,
                        center = Offset(bx, by),
                    )
                    drawCircle(
                        color = if (isSel) theme.textOnAccent else theme.accent,
                        radius = if (isSel) 9.5f else 6.0f,
                        center = Offset(bx, by),
                        style = Stroke(width = 2.2f),
                    )
                }
            }
        }

        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 6.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            listOf("20", "60", "200", "1K", "4K", "16K").forEach { lbl ->
                Text(
                    text = lbl,
                    color = theme.textSubtle,
                    style = MaterialTheme.typography.labelSmall,
                )
            }
        }
    }
}
