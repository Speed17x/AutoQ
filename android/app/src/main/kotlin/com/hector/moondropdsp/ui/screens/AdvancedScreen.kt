package com.hector.moondropdsp.ui.screens

import android.appwidget.AppWidgetManager
import android.content.ClipData
import android.content.ClipboardManager
import android.content.ComponentName
import android.content.Context
import android.widget.Toast
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.hector.moondropdsp.data.EqPreset
import com.hector.moondropdsp.hardware.DeviceKind
import com.hector.moondropdsp.ui.MoondropUiState
import com.hector.moondropdsp.ui.components.DotMatrixText
import com.hector.moondropdsp.ui.components.NothingCard
import com.hector.moondropdsp.ui.components.NothingOutlineButton
import com.hector.moondropdsp.ui.components.NothingPillChip
import com.hector.moondropdsp.ui.components.NothingPrimaryButton
import com.hector.moondropdsp.ui.components.NothingSectionLabel
import com.hector.moondropdsp.ui.components.NothingStatusDot
import com.hector.moondropdsp.ui.theme.NothingBorder
import com.hector.moondropdsp.ui.theme.NothingBorderStrong
import com.hector.moondropdsp.ui.theme.NothingEmerald
import com.hector.moondropdsp.ui.theme.NothingGraphBg
import com.hector.moondropdsp.ui.theme.NothingMuted
import com.hector.moondropdsp.ui.theme.NothingRed
import com.hector.moondropdsp.ui.theme.NothingSubtle
import com.hector.moondropdsp.ui.theme.NothingSurface
import com.hector.moondropdsp.ui.theme.NothingSurfaceHigh
import com.hector.moondropdsp.ui.theme.NothingWhite
import com.hector.moondropdsp.widget.NothingEqWidgetProvider

@Composable
fun AdvancedScreen(
    state: MoondropUiState,
    onApplyPreset: (EqPreset) -> Unit,
    onSavePreset: (String) -> Unit,
    onDeletePreset: (String) -> Unit,
    onExportApo: () -> String,
    onImportApo: (String) -> Unit,
    onSetBtGlobalEq: (Boolean) -> Unit,
    onSetBtSessionZero: (Boolean) -> Unit,
    onRequestBluetoothPermission: () -> Unit,
    onBtSppRead: () -> Unit,
    onBtSppWrite: () -> Unit,
    onBtSppFirmwarePreset: (Int, String) -> Unit,
    onRefreshDiagnostics: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    var newPresetName by remember { mutableStateOf("") }
    var showImportDialog by remember { mutableStateOf(false) }
    var importText by remember { mutableStateOf("") }

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        // Header
        NothingCard {
            DotMatrixText(text = "AVANZADO", dotSize = 2.4.dp, dotGap = 1.0.dp)
            Spacer(Modifier.height(4.dp))
            Text(
                text = "PRESETS · WIDGET NOTHING OS · BLUETOOTH DSP · DIAGNÓSTICO",
                color = NothingMuted,
                style = MaterialTheme.typography.labelSmall,
            )
        }

        // 1. Nothing OS Widget Pinning Card
        NothingCard {
            NothingSectionLabel(text = "Widget de escritorio (Nothing OS)", trailing = "4×2 · Redimensionable")
            Spacer(Modifier.height(8.dp))
            Text(
                text = "Control rápido con tipografía dot-matrix, gráfica en vivo, botón ON/OFF y cambio directo entre tus presets guardados.",
                color = NothingMuted,
                style = MaterialTheme.typography.bodyMedium,
            )
            Spacer(Modifier.height(10.dp))
            NothingPrimaryButton(
                text = "Añadir Widget a Pantalla de Inicio",
                onClick = {
                    val mgr = AppWidgetManager.getInstance(context)
                    val provider = ComponentName(context, NothingEqWidgetProvider::class.java)
                    if (mgr.isRequestPinAppWidgetSupported) {
                        NothingEqWidgetProvider.updateAllWidgets(context)
                        mgr.requestPinAppWidget(provider, null, null)
                    } else {
                        Toast.makeText(
                            context,
                            "Añade 'AutoQ · Nothing' manteniendo pulsada la pantalla de inicio.",
                            Toast.LENGTH_LONG,
                        ).show()
                    }
                },
                modifier = Modifier.fillMaxWidth(),
                accentRed = true,
            )
        }

        // 2. Presets Manager (synced with Widget)
        NothingCard {
            NothingSectionLabel(
                text = "Presets guardados",
                trailing = "${state.presets.size} disponibles",
            )
            Spacer(Modifier.height(10.dp))

            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                state.presets.forEach { preset ->
                    val active = preset.name == state.activePresetName
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(14.dp))
                            .background(if (active) NothingSurfaceHigh else NothingGraphBg)
                            .border(1.dp, if (active) NothingRed else NothingBorder, RoundedCornerShape(14.dp))
                            .clickable { onApplyPreset(preset) }
                            .padding(horizontal = 12.dp, vertical = 10.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.SpaceBetween,
                    ) {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                            modifier = Modifier.weight(1f),
                        ) {
                            NothingStatusDot(active = active, size = 7.dp)
                            Text(
                                text = preset.name,
                                color = if (active) NothingWhite else NothingMuted,
                                style = MaterialTheme.typography.bodyMedium,
                                fontWeight = if (active) FontWeight.Bold else FontWeight.Normal,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                        if (!preset.builtIn) {
                            Text(
                                text = "ELIMINAR",
                                color = NothingRed,
                                style = MaterialTheme.typography.labelSmall,
                                modifier = Modifier
                                    .clickable { onDeletePreset(preset.name) }
                                    .padding(horizontal = 6.dp, vertical = 4.dp),
                            )
                        } else {
                            Text(
                                text = "BASE",
                                color = NothingSubtle,
                                style = MaterialTheme.typography.labelSmall,
                            )
                        }
                    }
                }
            }

            Spacer(Modifier.height(10.dp))
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                OutlinedTextField(
                    value = newPresetName,
                    onValueChange = { newPresetName = it },
                    placeholder = { Text("Nombre del nuevo preset...", color = NothingSubtle, fontSize = 13.sp) },
                    singleLine = true,
                    modifier = Modifier.weight(1f),
                    shape = RoundedCornerShape(14.dp),
                    colors = OutlinedTextFieldDefaults.colors(
                        focusedBorderColor = NothingRed,
                        unfocusedBorderColor = NothingBorderStrong,
                        focusedTextColor = NothingWhite,
                        unfocusedTextColor = NothingWhite,
                        focusedContainerColor = NothingGraphBg,
                        unfocusedContainerColor = NothingGraphBg,
                    ),
                )
                NothingPrimaryButton(
                    text = "Guardar",
                    onClick = {
                        onSavePreset(newPresetName)
                        newPresetName = ""
                    },
                    enabled = newPresetName.isNotBlank(),
                    accentRed = false,
                )
            }
        }

        // 3. Bluetooth Engine (DynamicsProcessing 32-band + SPP RFCOMM)
        NothingCard {
            NothingSectionLabel(
                text = "Bluetooth · EQ Global & SPP",
                trailing = if (state.btServiceRunning) "ACTIVO (${state.btAttachedSessions} ses.)" else "DETENIDO",
            )
            Spacer(Modifier.height(10.dp))

            if (!state.hasBluetoothPermission) {
                NothingPrimaryButton(
                    text = "Conceder permiso Bluetooth",
                    onClick = onRequestBluetoothPermission,
                    modifier = Modifier.fillMaxWidth(),
                )
                Spacer(Modifier.height(10.dp))
            }

            ToggleSettingRow(
                title = "EQ Global DynamicsProcessing (32 bandas)",
                subtitle = "Aplica la curva paramétrica a cualquier audífono Bluetooth, DAC USB-C o 3.5mm sin root.",
                checked = state.btGlobalEqEnabled,
                onCheckedChange = onSetBtGlobalEq,
            )
            Spacer(Modifier.height(8.dp))
            ToggleSettingRow(
                title = "Fallback Sesión Global (0)",
                subtitle = "Además de sesiones por app (Spotify/YT Music), intenta adjuntarse a la mezcla global.",
                checked = state.btSessionZeroFallback,
                onCheckedChange = onSetBtSessionZero,
            )

            state.btServiceError?.let { err ->
                Spacer(Modifier.height(6.dp))
                Text(text = err, color = NothingRed, style = MaterialTheme.typography.labelSmall)
            }

            Spacer(Modifier.height(12.dp))
            NothingSectionLabel(
                text = "DSP Directo por Bluetooth SPP (TWS)",
                trailing = if (state.activeDevice?.kind == DeviceKind.BT_AUDIO) state.activeDevice.btMac.ifBlank { "Sin MAC" } else "BT desconectado",
            )
            Spacer(Modifier.height(8.dp))
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                NothingOutlineButton(
                    text = "SPP Leer",
                    onClick = onBtSppRead,
                    enabled = !state.busy && state.activeDevice?.kind == DeviceKind.BT_AUDIO,
                    modifier = Modifier.weight(1f),
                )
                NothingPrimaryButton(
                    text = "SPP Escribir",
                    onClick = onBtSppWrite,
                    enabled = !state.busy && state.activeDevice?.kind == DeviceKind.BT_AUDIO,
                    modifier = Modifier.weight(1f),
                )
            }
            Spacer(Modifier.height(8.dp))
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                listOf(0 to "POP / STD", 1 to "MONITOR", 2 to "BASSHEAD").forEach { (idx, label) ->
                    NothingPillChip(
                        label = label,
                        selected = false,
                        onClick = { onBtSppFirmwarePreset(idx, label) },
                        modifier = Modifier.weight(1f),
                    )
                }
            }
        }

        // 4. EqualizerAPO Import / Export
        NothingCard {
            NothingSectionLabel(text = "EqualizerAPO ParametricEQ.txt")
            Spacer(Modifier.height(10.dp))
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                NothingOutlineButton(
                    text = "Importar texto",
                    onClick = { showImportDialog = true },
                    modifier = Modifier.weight(1f),
                )
                NothingOutlineButton(
                    text = "Copiar APO",
                    onClick = {
                        val txt = onExportApo()
                        val cm = context.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
                        cm?.setPrimaryClip(ClipData.newPlainText("Moondrop ParametricEQ", txt))
                        Toast.makeText(context, "ParametricEQ copiado al portapapeles", Toast.LENGTH_SHORT).show()
                    },
                    modifier = Modifier.weight(1f),
                )
            }
        }

        // 5. Hardware Diagnostics
        NothingCard {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                NothingSectionLabel(text = "Diagnóstico USB y Audio")
            }
            Spacer(Modifier.height(8.dp))

            if (state.usbDiagnostics.isEmpty()) {
                Text(
                    text = "USB Host: sin dispositivos USB-C conectados.",
                    color = NothingSubtle,
                    fontFamily = FontFamily.Monospace,
                    fontSize = 11.sp,
                )
            } else {
                state.usbDiagnostics.forEach { u ->
                    Text(
                        text = "• ${u.productName} [${u.vidPid}] · HID=${u.hasHidInterface} · Perm=${u.hasPermission}" +
                            (u.recognizedAs?.let { " → $it" } ?: ""),
                        color = if (u.recognizedAs != null) NothingEmerald else NothingMuted,
                        fontFamily = FontFamily.Monospace,
                        fontSize = 11.sp,
                    )
                }
            }

            Spacer(Modifier.height(6.dp))
            if (state.audioOutputDiagnostics.isEmpty()) {
                Text(
                    text = "Salidas externas: solo altavoz interno.",
                    color = NothingSubtle,
                    fontFamily = FontFamily.Monospace,
                    fontSize = 11.sp,
                )
            } else {
                state.audioOutputDiagnostics.forEach { out ->
                    Text(
                        text = "• Audio: $out",
                        color = NothingMuted,
                        fontFamily = FontFamily.Monospace,
                        fontSize = 11.sp,
                    )
                }
            }

            Spacer(Modifier.height(10.dp))
            NothingOutlineButton(
                text = "↻ Re-escanear hardware",
                onClick = onRefreshDiagnostics,
                modifier = Modifier.fillMaxWidth(),
            )
        }

        Text(
            text = "> ${state.statusMessage}",
            color = if (state.statusError) NothingRed else NothingMuted,
            fontFamily = FontFamily.Monospace,
            fontSize = 11.sp,
            modifier = Modifier.padding(horizontal = 4.dp),
        )
        Spacer(Modifier.height(12.dp))
    }

    if (showImportDialog) {
        AlertDialog(
            onDismissRequest = { showImportDialog = false },
            containerColor = NothingSurface,
            titleContentColor = NothingWhite,
            title = { Text("Importar ParametricEQ", fontFamily = FontFamily.Monospace) },
            text = {
                OutlinedTextField(
                    value = importText,
                    onValueChange = { importText = it },
                    placeholder = {
                        Text(
                            "Filter 1: ON PK Fc 105 Hz Gain 3.2 dB Q 0.707",
                            color = NothingSubtle,
                            fontFamily = FontFamily.Monospace,
                            fontSize = 11.sp,
                        )
                    },
                    minLines = 5,
                    maxLines = 8,
                    colors = OutlinedTextFieldDefaults.colors(
                        focusedBorderColor = NothingRed,
                        unfocusedBorderColor = NothingBorderStrong,
                        focusedTextColor = NothingWhite,
                        unfocusedTextColor = NothingWhite,
                    ),
                )
            },
            confirmButton = {
                TextButton(
                    onClick = {
                        onImportApo(importText)
                        showImportDialog = false
                        importText = ""
                    },
                ) {
                    Text("IMPORTAR", color = NothingRed, fontFamily = FontFamily.Monospace, fontWeight = FontWeight.Bold)
                }
            },
            dismissButton = {
                TextButton(onClick = { showImportDialog = false }) {
                    Text("CANCELAR", color = NothingMuted, fontFamily = FontFamily.Monospace)
                }
            },
        )
    }
}

@Composable
private fun ToggleSettingRow(
    title: String,
    subtitle: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Column(modifier = Modifier.weight(1f).padding(end = 10.dp)) {
            Text(text = title, color = NothingWhite, style = MaterialTheme.typography.bodyMedium, fontWeight = FontWeight.SemiBold)
            Text(text = subtitle, color = NothingSubtle, style = MaterialTheme.typography.labelSmall)
        }
        Switch(
            checked = checked,
            onCheckedChange = onCheckedChange,
            colors = SwitchDefaults.colors(
                checkedThumbColor = NothingWhite,
                checkedTrackColor = NothingRed,
                uncheckedThumbColor = NothingMuted,
                uncheckedTrackColor = NothingSurfaceHigh,
                uncheckedBorderColor = NothingBorderStrong,
            ),
        )
    }
}
