package com.hector.moondropdsp.ui.screens

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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.hector.moondropdsp.dsp.BiquadMath
import com.hector.moondropdsp.dsp.FilterBand
import com.hector.moondropdsp.dsp.FilterType
import com.hector.moondropdsp.hardware.DeviceKind
import com.hector.moondropdsp.hardware.MoondropDevice
import com.hector.moondropdsp.ui.MoondropUiState
import com.hector.moondropdsp.ui.components.DotMatrixText
import com.hector.moondropdsp.ui.components.InteractiveEqCurveCanvas
import com.hector.moondropdsp.ui.components.NothingCard
import com.hector.moondropdsp.ui.components.NothingOutlineButton
import com.hector.moondropdsp.ui.components.NothingPillChip
import com.hector.moondropdsp.ui.components.NothingPrimaryButton
import com.hector.moondropdsp.ui.components.NothingSectionLabel
import com.hector.moondropdsp.ui.components.NothingStatusDot
import com.hector.moondropdsp.ui.theme.NothingBorder
import com.hector.moondropdsp.ui.theme.NothingBorderStrong
import com.hector.moondropdsp.ui.theme.NothingMuted
import com.hector.moondropdsp.ui.theme.NothingRed
import com.hector.moondropdsp.ui.theme.NothingSubtle
import com.hector.moondropdsp.ui.theme.NothingSurfaceHigh
import com.hector.moondropdsp.ui.theme.NothingWhite
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
    modifier: Modifier = Modifier,
) {
    val dev = state.activeDevice
    val maxGain = dev?.maxGainDb ?: 12.0
    val allowShelves = dev?.supportsShelves ?: true
    val selectedBand = state.bands.getOrElse(state.selectedBandIdx) { FilterBand() }

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        // 1. Device Status Card (Nothing OS style)
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
                            text = (dev?.shortWidgetName ?: "AUTOQ").take(14),
                            color = if (dev != null) NothingWhite else NothingMuted,
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
                            color = NothingMuted,
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
                    text = "Autorizar permiso USB (${dev.vidPid})",
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
                    label = if (state.overlayFrMode) "CURVA: FR" else "CURVA: ΔEQ",
                    selected = state.overlayFrMode,
                    onClick = onToggleOverlayFr,
                    accentRed = false,
                    modifier = Modifier.weight(1f),
                )
                NothingPillChip(
                    label = "PLANO 0dB",
                    selected = false,
                    onClick = onResetFlat,
                    modifier = Modifier.weight(1f),
                )
                NothingPillChip(
                    label = "↻ LEER",
                    selected = false,
                    onClick = onRefresh,
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
                        .clip(RoundedCornerShape(16.dp))
                        .background(if (isSel) NothingSurfaceHigh else Color(0xFF121316))
                        .border(
                            width = 1.dp,
                            color = if (isSel) NothingRed else NothingBorder,
                            shape = RoundedCornerShape(16.dp),
                        )
                        .clickable { onSelectBand(idx) }
                        .padding(horizontal = 12.dp, vertical = 8.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Text(
                        text = "B${idx + 1} · ${band.type.shortCode}",
                        color = if (isSel) NothingRed else NothingSubtle,
                        style = MaterialTheme.typography.labelSmall,
                    )
                    Text(
                        text = BiquadMath.formatFreq(band.frequency),
                        color = NothingWhite,
                        style = MaterialTheme.typography.labelMedium,
                        fontWeight = FontWeight.Bold,
                    )
                    Text(
                        text = "${gainStr}dB",
                        color = if (band.gain != 0.0) NothingWhite else NothingMuted,
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
                            checkedThumbColor = NothingWhite,
                            checkedTrackColor = NothingRed,
                            uncheckedThumbColor = NothingMuted,
                            uncheckedTrackColor = NothingSurfaceHigh,
                            uncheckedBorderColor = NothingBorderStrong,
                        ),
                    )
                    Column {
                        Text(
                            text = "LIVE",
                            color = NothingWhite,
                            style = MaterialTheme.typography.labelLarge,
                        )
                        Text(
                            text = "Envío inmediato",
                            color = NothingSubtle,
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
        }

        // Status footer
        Text(
            text = "> ${state.statusMessage}",
            color = if (state.statusError) NothingRed else NothingMuted,
            fontFamily = FontFamily.Monospace,
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
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Text(
            text = label,
            color = NothingMuted,
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
                color = NothingWhite,
                style = MaterialTheme.typography.labelMedium,
                fontWeight = FontWeight.Bold,
            )
            StepCircleButton("+", onPlus)
        }
    }
}

@Composable
private fun StepCircleButton(symbol: String, onClick: () -> Unit) {
    Box(
        modifier = Modifier
            .clip(CircleShape)
            .background(NothingSurfaceHigh)
            .border(1.dp, NothingBorderStrong, CircleShape)
            .clickable(onClick = onClick)
            .width(28.dp)
            .height(24.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = symbol,
            color = NothingWhite,
            fontFamily = FontFamily.Monospace,
            fontWeight = FontWeight.Bold,
            fontSize = 13.sp,
        )
    }
}

@Composable
private fun nothingSliderColors() = SliderDefaults.colors(
    thumbColor = NothingWhite,
    activeTrackColor = NothingRed,
    inactiveTrackColor = NothingBorderStrong,
)
