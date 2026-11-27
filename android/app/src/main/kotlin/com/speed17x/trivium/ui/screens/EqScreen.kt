package com.speed17x.trivium.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.speed17x.trivium.dsp.BiquadMath
import com.speed17x.trivium.dsp.FilterBand
import com.speed17x.trivium.dsp.FilterType
import com.speed17x.trivium.hardware.DeviceKind
import com.speed17x.trivium.hardware.MoondropDevice
import com.speed17x.trivium.ui.MoondropUiState
import com.speed17x.trivium.ui.components.DotMatrixText
import com.speed17x.trivium.ui.UiLang
import com.speed17x.trivium.ui.components.InteractiveEqCurveCanvas
import com.speed17x.trivium.ui.components.NothingCard
import com.speed17x.trivium.ui.components.NothingOutlineButton
import com.speed17x.trivium.ui.components.NothingPillChip
import com.speed17x.trivium.ui.components.NothingPrimaryButton
import com.speed17x.trivium.ui.components.NothingSectionLabel
import com.speed17x.trivium.ui.components.NothingStatusDot
import com.speed17x.trivium.ui.theme.ConsolasFontFamily
import com.speed17x.trivium.ui.theme.LocalTriviumTheme
import java.util.Locale
import kotlin.math.log10
import kotlin.math.pow
import kotlin.math.round

@Composable
fun EqScreen(
    state: MoondropUiState,
    onSelectDevice: (MoondropDevice) -> Unit,
    onRequestUsbPermission: () -> Unit,
    onRefresh: () -> Unit,
    onToggleDsp: () -> Unit,
    onToggleOverlayFr: () -> Unit,
    onResetFlat: () -> Unit,
    onSelectBand: (Int) -> Unit,
    onUpdateBand: (Int, (FilterBand) -> FilterBand) -> Unit,
    onDragBand: (Int, Double, Double) -> Unit,
    onDragEnd: () -> Unit,
    onToggleLive: (Boolean) -> Unit,
    onApply: (commit: Boolean) -> Unit,
    onSetPreampDb: (Double) -> Unit = {},
    onSetAutoPreamp: (Boolean) -> Unit = {},
    onSetOutputGainDb: (Double) -> Unit = {},
    onResetOutputGain: () -> Unit = {},
    onSelectAbSlot: (Char) -> Unit = {},
    onCopyToOtherAbSlot: () -> Unit = {},
    onPowerOffAndExit: () -> Unit = {},
    modifier: Modifier = Modifier,
) {
    val dev = state.activeDevice
    val maxGain = dev?.maxGainDb ?: 12.0
    val allowShelves = dev?.supportsShelves ?: true
    val selectedBand = state.bands.getOrElse(state.selectedBandIdx) { FilterBand() }
    val t = LocalTriviumTheme.current
    val bandChipRadius = (t.cardRadiusDp - 2).coerceAtLeast(10).dp

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        // 1. Device Status Card
        NothingCard {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                    modifier = Modifier.weight(1f),
                ) {
                    NothingStatusDot(active = dev != null && state.dspEnabled, size = 9.dp)
                    Column {
                        DotMatrixText(
                            text = (dev?.shortWidgetName ?: "TRIVIUM").take(14),
                            color = if (dev != null) t.textPrimary else t.textMuted,
                            dotSize = 2.4.dp,
                            dotGap = 1.0.dp,
                        )
                        Spacer(Modifier.height(4.dp))
                        Text(
                            text = if (dev != null) {
                                "${dev.transportBadge} · ${dev.maxBands} BANDAS · ${state.activePresetName.uppercase()}"
                            } else {
                                "CONECTA CUALQUIER AUDÍFONO BT, USB-C O 3.5MM"
                            },
                            color = t.textMuted,
                            style = MaterialTheme.typography.labelSmall,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
                NothingPillChip(
                    label = if (state.dspEnabled) "ON" else "OFF",
                    selected = state.dspEnabled,
                    onClick = onToggleDsp,
                    accentRed = true,
                )
            }

            if (state.devices.size > 1) {
                Spacer(Modifier.height(10.dp))
                Row(
                    modifier = Modifier.horizontalScroll(rememberScrollState()),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    state.devices.forEach { d ->
                        NothingPillChip(
                            label = "${d.shortWidgetName} (${d.transportBadge})",
                            selected = d.id == dev?.id,
                            onClick = { onSelectDevice(d) },
                            accentRed = false,
                        )
                    }
                }
            }

            if (dev != null && dev.kind == DeviceKind.USB_HID && !dev.hasUsbPermission) {
                Spacer(Modifier.height(10.dp))
                NothingPrimaryButton(
                    text = UiLang.tr("Autorizar permiso USB (${dev.vidPid})", "Grant USB permission (${dev.vidPid})"),
                    onClick = onRequestUsbPermission,
                    modifier = Modifier.fillMaxWidth(),
                )
            }

            Spacer(Modifier.height(10.dp))
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                NothingPillChip(
                    label = if (state.overlayFrMode) UiLang.tr("CURVA: FR", "CURVE: FR") else UiLang.tr("CURVA: ΔEQ", "CURVE: ΔEQ"),
                    selected = state.overlayFrMode,
                    onClick = onToggleOverlayFr,
                    accentRed = false,
                    modifier = Modifier.weight(1f),
                )
                NothingPillChip(
                    label = UiLang.tr("PLANO 0dB", "FLAT 0dB"),
                    selected = false,
                    onClick = onResetFlat,
                    modifier = Modifier.weight(1f),
                )
                NothingPillChip(
                    label = UiLang.tr("↻ LEER", "↻ READ"),
                    selected = false,
                    onClick = onRefresh,
                    modifier = Modifier.weight(0.85f),
                )
            }
            Spacer(Modifier.height(8.dp))
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    text = "A/B",
                    color = t.textMuted,
                    style = MaterialTheme.typography.labelSmall,
                    fontWeight = FontWeight.Bold,
                )
                NothingPillChip(
                    label = "SLOT A",
                    selected = state.activeAbSlot == 'A',
                    onClick = { onSelectAbSlot('A') },
                    accentRed = true,
                    modifier = Modifier.weight(1f),
                )
                NothingPillChip(
                    label = "SLOT B",
                    selected = state.activeAbSlot == 'B',
                    onClick = { onSelectAbSlot('B') },
                    accentRed = true,
                    modifier = Modifier.weight(1f),
                )
                NothingPillChip(
                    label = if (state.activeAbSlot == 'A') "A → B" else "B → A",
                    selected = false,
                    onClick = onCopyToOtherAbSlot,
                    modifier = Modifier.weight(0.85f),
                )
            }
        }

        // 2. Superposed Interactive EQ/FR Curve
        InteractiveEqCurveCanvas(
            bands = state.bands,
            selectedBandIdx = state.selectedBandIdx,
            dspEnabled = state.dspEnabled,
            overlayFrMode = state.overlayFrMode,
            maxGainDb = maxGain,
            stockFr = state.stockFrCurve,
            targetFr = state.targetFrCurve,
            targetLabel = state.targetLabel,
            onSelectBand = onSelectBand,
            onDragBand = onDragBand,
            onDragEnd = onDragEnd,
        )

        // 2b. Manual Preamp, Output Gain/Attenuation & Anti-Clipping Headroom Card
        val effPreamp = if (state.autoPreamp) {
            BiquadMath.computeSafePreampDb(state.bands)
        } else {
            state.preampDb.coerceIn(-18.0, 6.0)
        }
        val peakBoost = BiquadMath.peakBoostDb(state.bands)
        val marginDb = BiquadMath.headroomMarginDb(state.bands, effPreamp)
        val headroomOk = marginDb >= 0.0
        val headroomColor = if (headroomOk) t.emerald else t.amber

        NothingCard {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    NothingSectionLabel(
                        text = "Preamp, Salida y Headroom",
                        trailing = String.format(Locale.US, "PICO +%.1f dB", peakBoost),
                    )
                    Spacer(Modifier.height(2.dp))
                    Text(
                        text = if (headroomOk) {
                            String.format(Locale.US, "MARGEN +%.1f dB · SIN CLIPPING", marginDb)
                        } else {
                            String.format(Locale.US, "⚠ CLIPPING POTENCIAL DE %.1f dB", -marginDb)
                        },
                        color = headroomColor,
                        fontFamily = ConsolasFontFamily,
                        fontSize = 10.sp,
                        fontWeight = FontWeight.Bold,
                    )
                }
                NothingPillChip(
                    label = if (state.autoPreamp) "AUTO ON" else "MANUAL",
                    selected = state.autoPreamp,
                    onClick = { onSetAutoPreamp(!state.autoPreamp) },
                    accentRed = false,
                )
            }
            Spacer(Modifier.height(8.dp))
            SliderHeaderRow(
                label = "PREAMP",
                valueText = String.format(Locale.US, "%+.1f dB", effPreamp),
                onMinus = { onSetPreampDb(effPreamp - 0.5) },
                onPlus = { onSetPreampDb(effPreamp + 0.5) },
            )
            Slider(
                value = effPreamp.toFloat().coerceIn(-18f, 6f),
                valueRange = -18f..6f,
                onValueChange = { v -> onSetPreampDb(v.toDouble()) },
                colors = nothingSliderColors(),
            )
            val usedFraction = ((peakBoost + effPreamp.coerceAtLeast(-18.0)).coerceIn(0.0, 12.0) / 12.0).toFloat()
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(6.dp)
                    .clip(CircleShape)
                    .background(t.insetBg),
            ) {
                Box(
                    modifier = Modifier
                        .fillMaxWidth(usedFraction.coerceIn(0.04f, 1f))
                        .height(6.dp)
                        .clip(CircleShape)
                        .background(headroomColor),
                )
            }

            Spacer(Modifier.height(12.dp))
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(1.dp)
                    .background(t.border),
            )
            Spacer(Modifier.height(10.dp))

            val outDb = state.outputGainDb.coerceIn(-30.0, 12.0)
            val outBadge = when {
                outDb < -0.05 -> "ATENUADO"
                outDb > 0.05 -> "BOOST"
                else -> "NEUTRO"
            }
            val outBadgeColor = when {
                outDb < -0.05 -> t.emerald
                outDb > 0.05 -> t.amber
                else -> t.textMuted
            }
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = "SALIDA / VOLUMEN POST-LIMITER",
                        color = t.textSubtle,
                        fontFamily = ConsolasFontFamily,
                        fontSize = 10.sp,
                        fontWeight = FontWeight.Bold,
                    )
                    Text(
                        text = "$outBadge · -30 dB a +12 dB (IEMs sensibles o ganancia extra)",
                        color = outBadgeColor,
                        fontFamily = ConsolasFontFamily,
                        fontSize = 10.sp,
                    )
                }
                NothingPillChip(
                    label = "0 dB",
                    selected = kotlin.math.abs(outDb) < 0.05,
                    onClick = onResetOutputGain,
                    accentRed = false,
                )
            }
            Spacer(Modifier.height(6.dp))
            SliderHeaderRow(
                label = "GANANCIA DE SALIDA",
                valueText = String.format(Locale.US, "%+.1f dB", outDb),
                onMinus = { onSetOutputGainDb(outDb - 1.0) },
                onPlus = { onSetOutputGainDb(outDb + 1.0) },
            )
            Slider(
                value = outDb.toFloat().coerceIn(-30f, 12f),
                valueRange = -30f..12f,
                onValueChange = { v -> onSetOutputGainDb((round(v * 10.0) / 10.0)) },
                colors = nothingSliderColors(),
            )
        }

        // 3. Band Selector Strip
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .horizontalScroll(rememberScrollState()),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            state.bands.forEachIndexed { idx, band ->
                val isSel = idx == state.selectedBandIdx
                val gainStr = String.format(Locale.US, "%+.1f", band.gain)
                Column(
                    modifier = Modifier
                        .clip(RoundedCornerShape(bandChipRadius))
                        .background(if (isSel) t.surfaceHigh else t.insetBg)
                        .border(
                            width = 1.dp,
                            color = if (isSel) t.accent else t.border,
                            shape = RoundedCornerShape(bandChipRadius),
                        )
                        .clickable { onSelectBand(idx) }
                        .padding(horizontal = 12.dp, vertical = 8.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Text(
                        text = "B${idx + 1} · ${band.type.shortCode}",
                        color = if (isSel) t.accent else t.textSubtle,
                        style = MaterialTheme.typography.labelSmall,
                    )
                    Text(
                        text = BiquadMath.formatFreq(band.frequency),
                        color = t.textPrimary,
                        style = MaterialTheme.typography.labelMedium,
                        fontWeight = FontWeight.Bold,
                    )
                    Text(
                        text = "${gainStr}dB",
                        color = if (band.gain != 0.0) t.textPrimary else t.textMuted,
                        style = MaterialTheme.typography.labelSmall,
                    )
                }
            }
        }

        // 4. Selected Band Fine Controls
        NothingCard {
            NothingSectionLabel(
                text = "Banda ${state.selectedBandIdx + 1}",
                trailing = "Q ${String.format(Locale.US, "%.2f", selectedBand.q)} · ${BiquadMath.formatFreq(selectedBand.frequency)}",
            )

            Spacer(Modifier.height(10.dp))

            // Filter type segmented row
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                FilterType.entries.forEach { ft ->
                    val allowed = allowShelves || ft == FilterType.PEAKING
                    NothingPillChip(
                        label = "${ft.shortCode} · ${ft.displayName}",
                        selected = selectedBand.type == ft,
                        onClick = {
                            if (allowed) {
                                onUpdateBand(state.selectedBandIdx) { it.copy(type = ft) }
                            }
                        },
                        accentRed = true,
                        modifier = Modifier.weight(1f),
                    )
                }
            }

            Spacer(Modifier.height(14.dp))

            // Frequency logarithmic slider + fine buttons
            val logMin = log10(20.0)
            val logMax = log10(20000.0)
            val freqNorm = ((log10(selectedBand.frequency.coerceIn(20.0, 20000.0)) - logMin) / (logMax - logMin)).toFloat()

            SliderHeaderRow(
                label = "FRECUENCIA",
                valueText = "${selectedBand.frequency.toInt()} Hz",
                onMinus = {
                    val step = if (selectedBand.frequency < 200) 5.0 else if (selectedBand.frequency < 2000) 25.0 else 100.0
                    onUpdateBand(state.selectedBandIdx) { it.copy(frequency = it.frequency - step) }
                },
                onPlus = {
                    val step = if (selectedBand.frequency < 200) 5.0 else if (selectedBand.frequency < 2000) 25.0 else 100.0
                    onUpdateBand(state.selectedBandIdx) { it.copy(frequency = it.frequency + step) }
                },
            )
            Slider(
                value = freqNorm.coerceIn(0f, 1f),
                onValueChange = { t ->
                    val hz = round(10.0.pow(logMin + t.toDouble() * (logMax - logMin)))
                    onUpdateBand(state.selectedBandIdx) { it.copy(frequency = hz) }
                },
                colors = nothingSliderColors(),
            )

            // Gain slider + fine buttons
            SliderHeaderRow(
                label = "GANANCIA",
                valueText = String.format(Locale.US, "%+.1f dB", selectedBand.gain),
                onMinus = { onUpdateBand(state.selectedBandIdx) { it.copy(gain = it.gain - 0.5) } },
                onPlus = { onUpdateBand(state.selectedBandIdx) { it.copy(gain = it.gain + 0.5) } },
            )
            Slider(
                value = selectedBand.gain.toFloat().coerceIn(-maxGain.toFloat(), maxGain.toFloat()),
                valueRange = -maxGain.toFloat()..maxGain.toFloat(),
                onValueChange = { g ->
                    val rounded = round(g * 10.0) / 10.0
                    onUpdateBand(state.selectedBandIdx) { it.copy(gain = rounded) }
                },
                colors = nothingSliderColors(),
            )

            // Q logarithmic slider + fine buttons
            val qLogMin = log10(0.1)
            val qLogMax = log10(10.0)
            val qNorm = ((log10(selectedBand.q.coerceIn(0.1, 10.0)) - qLogMin) / (qLogMax - qLogMin)).toFloat()

            SliderHeaderRow(
                label = "FACTOR Q",
                valueText = String.format(Locale.US, "%.2f", selectedBand.q),
                onMinus = { onUpdateBand(state.selectedBandIdx) { it.copy(q = it.q - 0.1) } },
                onPlus = { onUpdateBand(state.selectedBandIdx) { it.copy(q = it.q + 0.1) } },
            )
            Slider(
                value = qNorm.coerceIn(0f, 1f),
                onValueChange = { t ->
                    val qVal = round(10.0.pow(qLogMin + t.toDouble() * (qLogMax - qLogMin)) * 100.0) / 100.0
                    onUpdateBand(state.selectedBandIdx) { it.copy(q = qVal) }
                },
                colors = nothingSliderColors(),
            )
        }

        // 5. Hardware Apply / Live / Flash Bar
        NothingCard {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Switch(
                        checked = state.livePreview,
                        onCheckedChange = onToggleLive,
                        colors = SwitchDefaults.colors(
                            checkedThumbColor = t.textOnAccent,
                            checkedTrackColor = t.accent,
                            uncheckedThumbColor = t.textMuted,
                            uncheckedTrackColor = t.surfaceHigh,
                            uncheckedBorderColor = t.borderStrong,
                        ),
                    )
                    Column {
                        Text(
                            text = "LIVE",
                            color = t.textPrimary,
                            style = MaterialTheme.typography.labelLarge,
                        )
                        Text(
                            text = "Envío inmediato",
                            color = t.textSubtle,
                            style = MaterialTheme.typography.labelSmall,
                        )
                    }
                }

                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    NothingOutlineButton(
                        text = "Aplicar",
                        onClick = { onApply(false) },
                        enabled = !state.busy,
                    )
                    if (dev == null || dev.kind == DeviceKind.USB_HID) {
                        NothingPrimaryButton(
                            text = "Flash",
                            onClick = { onApply(true) },
                            enabled = !state.busy && dev != null && dev.hasUsbPermission,
                            accentRed = true,
                        )
                    }
                }
            }
            Spacer(Modifier.height(10.dp))
            NothingOutlineButton(
                text = "⏻ Apagar EQ y detener app",
                onClick = onPowerOffAndExit,
                enabled = !state.busy,
                modifier = Modifier.fillMaxWidth(),
            )
        }

        // Status footer
        Text(
            text = "> ${state.statusMessage}",
            color = if (state.statusError) t.accent else t.textMuted,
            fontFamily = ConsolasFontFamily,
            fontSize = 11.sp,
            modifier = Modifier.padding(horizontal = 4.dp, vertical = 2.dp),
        )
        Spacer(Modifier.height(12.dp))
    }
}

@Composable
private fun SliderHeaderRow(
    label: String,
    valueText: String,
    onMinus: () -> Unit,
    onPlus: () -> Unit,
) {
    val t = LocalTriviumTheme.current
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Text(
            text = label,
            color = t.textMuted,
            style = MaterialTheme.typography.labelSmall,
            letterSpacing = 1.sp,
        )
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            StepCircleButton("−", onMinus)
            Text(
                text = valueText,
                color = t.textPrimary,
                style = MaterialTheme.typography.labelMedium,
                fontWeight = FontWeight.Bold,
            )
            StepCircleButton("+", onPlus)
        }
    }
}

@Composable
private fun StepCircleButton(symbol: String, onClick: () -> Unit) {
    val t = LocalTriviumTheme.current
    Box(
        modifier = Modifier
            .clip(CircleShape)
            .background(t.surfaceHigh)
            .border(1.dp, t.borderStrong, CircleShape)
            .clickable(onClick = onClick)
            .width(28.dp)
            .height(24.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = symbol,
            color = t.textPrimary,
            fontFamily = ConsolasFontFamily,
            fontWeight = FontWeight.Bold,
            fontSize = 13.sp,
        )
    }
}

@Composable
private fun nothingSliderColors() = LocalTriviumTheme.current.let { t ->
    SliderDefaults.colors(
        thumbColor = t.accent,
        activeTrackColor = t.accent,
        inactiveTrackColor = t.borderStrong,
    )
}
