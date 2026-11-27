package com.speed17x.trivium.ui.screens

import android.appwidget.AppWidgetManager
import android.content.ClipData
import android.content.ClipboardManager
import android.content.ComponentName
import android.content.Context
import android.widget.Toast
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
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.speed17x.trivium.data.EqPreset
import com.speed17x.trivium.hardware.DeviceKind
import com.speed17x.trivium.ui.MoondropUiState
import com.speed17x.trivium.ui.UiLang
import com.speed17x.trivium.ui.components.DotMatrixText
import com.speed17x.trivium.ui.components.NothingCard
import com.speed17x.trivium.ui.components.NothingOutlineButton
import com.speed17x.trivium.ui.components.NothingPillChip
import com.speed17x.trivium.ui.components.NothingPrimaryButton
import com.speed17x.trivium.ui.components.NothingSectionLabel
import com.speed17x.trivium.ui.components.NothingStatusDot
import com.speed17x.trivium.ui.theme.AppAccentId
import com.speed17x.trivium.ui.theme.AppThemeId
import com.speed17x.trivium.ui.theme.ConsolasFontFamily
import com.speed17x.trivium.ui.theme.LocalTriviumTheme
import com.speed17x.trivium.ui.theme.tokensFor
import com.speed17x.trivium.widget.NothingEqWidgetProvider

@Composable
fun AdvancedScreen(
    state: MoondropUiState,
    onSelectTheme: (AppThemeId) -> Unit = {},
    onSelectAccent: (AppAccentId) -> Unit = {},
    onApplyPreset: (EqPreset) -> Unit,
    onSavePreset: (String) -> Unit,
    onDeletePreset: (String) -> Unit,
    onExportApo: () -> String,
    onExportFormat: (String) -> String = { onExportApo() },
    onImportApo: (String) -> Unit,
    onSetBtGlobalEq: (Boolean) -> Unit,
    onSetBtSessionZero: (Boolean) -> Unit,
    onRequestBluetoothPermission: () -> Unit,
    onBtSppRead: () -> Unit,
    onBtSppWrite: () -> Unit,
    onBtSppFirmwarePreset: (Int, String) -> Unit,
    onRefreshDiagnostics: () -> Unit,
    onPowerOffAndExit: () -> Unit = {},
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val t = LocalTriviumTheme.current
    var newPresetName by remember { mutableStateOf("") }
    var showImportDialog by remember { mutableStateOf(false) }
    var importText by remember { mutableStateOf("") }
    var selectedExportFmt by remember { mutableStateOf("apo") }

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        // Header
        NothingCard {
            DotMatrixText(text = UiLang.tr("AVANZADO", "ADVANCED"), dotSize = 2.4.dp, dotGap = 1.0.dp)
            Spacer(Modifier.height(4.dp))
            Text(
                text = UiLang.tr(
                    "TEMAS · PRESETS · WIDGET · BLUETOOTH DSP · DIAGNÓSTICO",
                    "THEMES · PRESETS · WIDGET · BLUETOOTH DSP · DIAGNOSTICS",
                ),
                color = t.textMuted,
                style = MaterialTheme.typography.labelSmall,
            )
        }

        // 0. Modular Theme Picker
        NothingCard {
            NothingSectionLabel(
                text = UiLang.tr("Apariencia y Temas", "Appearance & Themes"),
                trailing = state.themeId.label,
            )
            Spacer(Modifier.height(8.dp))
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                AppThemeId.entries.forEach { option ->
                    val selected = option == state.themeId
                    val itemShape = RoundedCornerShape((t.cardRadiusDp - 4).coerceAtLeast(10).dp)
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(itemShape)
                            .background(if (selected) t.surfaceHigh else t.insetBg)
                            .border(
                                width = 1.dp,
                                color = if (selected) t.accent else t.border,
                                shape = itemShape,
                            )
                            .clickable { onSelectTheme(option) }
                            .padding(horizontal = 12.dp, vertical = 10.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.SpaceBetween,
                    ) {
                        Column(modifier = Modifier.weight(1f).padding(end = 8.dp)) {
                            Text(
                                text = option.label,
                                color = if (selected) t.textPrimary else t.textMuted,
                                style = MaterialTheme.typography.bodyMedium,
                                fontWeight = if (selected) FontWeight.Bold else FontWeight.Normal,
                            )
                            Text(
                                text = option.subtitle,
                                color = t.textSubtle,
                                style = MaterialTheme.typography.labelSmall,
                            )
                        }
                        NothingStatusDot(active = selected, size = 8.dp)
                    }
                }
            }
            Spacer(Modifier.height(10.dp))
            NothingSectionLabel(
                text = UiLang.tr("Color de resaltado", "Accent Color"),
                trailing = state.accentId.label,
            )
            Spacer(Modifier.height(8.dp))
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .horizontalScroll(rememberScrollState()),
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                AppAccentId.entries.forEach { acc ->
                    val selected = acc == state.accentId
                    val swatchColor = acc.resolveColor(t.isDark) ?: tokensFor(state.themeId).accent
                    Column(
                        horizontalAlignment = Alignment.CenterHorizontally,
                        modifier = Modifier
                            .clip(RoundedCornerShape(10.dp))
                            .clickable { onSelectAccent(acc) }
                            .padding(horizontal = 4.dp, vertical = 2.dp),
                    ) {
                        Box(
                            modifier = Modifier
                                .size(34.dp)
                                .clip(CircleShape)
                                .border(
                                    width = if (selected) 2.dp else 1.dp,
                                    color = if (selected) t.textPrimary else t.border,
                                    shape = CircleShape,
                                ),
                            contentAlignment = Alignment.Center,
                        ) {
                            Box(
                                modifier = Modifier
                                    .size(24.dp)
                                    .clip(CircleShape)
                                    .background(swatchColor),
                                contentAlignment = Alignment.Center,
                            ) {
                                if (acc == AppAccentId.AUTO) {
                                    Text(
                                        text = "T",
                                        color = t.textOnAccent,
                                        fontFamily = ConsolasFontFamily,
                                        fontWeight = FontWeight.Bold,
                                        fontSize = 11.sp,
                                    )
                                }
                            }
                        }
                        Spacer(Modifier.height(3.dp))
                        Text(
                            text = acc.label,
                            color = if (selected) t.textPrimary else t.textMuted,
                            style = MaterialTheme.typography.labelSmall,
                            fontWeight = if (selected) FontWeight.Bold else FontWeight.Normal,
                            maxLines = 1,
                        )
                    }
                }
            }
        }

        // 1. Desktop Widget Pinning Card
        NothingCard {
            NothingSectionLabel(text = "Widget de escritorio", trailing = "4×2 · Redimensionable")
            Spacer(Modifier.height(8.dp))
            Text(
                text = "Control rápido en pantalla de inicio con gráfica en vivo, botón ON/OFF y cambio directo entre presets guardados.",
                color = t.textMuted,
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
                            "Añade el widget de Trivium manteniendo pulsada la pantalla de inicio.",
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
                            .background(if (active) t.surfaceHigh else t.insetBg)
                            .border(1.dp, if (active) t.accent else t.border, RoundedCornerShape(14.dp))
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
                                color = if (active) t.textPrimary else t.textMuted,
                                style = MaterialTheme.typography.bodyMedium,
                                fontWeight = if (active) FontWeight.Bold else FontWeight.Normal,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                        if (!preset.builtIn) {
                            Text(
                                text = "ELIMINAR",
                                color = t.accent,
                                style = MaterialTheme.typography.labelSmall,
                                modifier = Modifier
                                    .clickable { onDeletePreset(preset.name) }
                                    .padding(horizontal = 6.dp, vertical = 4.dp),
                            )
                        } else {
                            Text(
                                text = "BASE",
                                color = t.textSubtle,
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
                    placeholder = { Text("Nombre del nuevo preset...", color = t.textSubtle, fontSize = 13.sp) },
                    singleLine = true,
                    modifier = Modifier.weight(1f),
                    shape = RoundedCornerShape(14.dp),
                    colors = OutlinedTextFieldDefaults.colors(
                        focusedBorderColor = t.accent,
                        unfocusedBorderColor = t.borderStrong,
                        focusedTextColor = t.textPrimary,
                        unfocusedTextColor = t.textPrimary,
                        focusedContainerColor = t.insetBg,
                        unfocusedContainerColor = t.insetBg,
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
                Text(text = err, color = t.accent, style = MaterialTheme.typography.labelSmall)
            }

            Spacer(Modifier.height(10.dp))
            NothingOutlineButton(
                text = "⏻ Apagar EQ y detener servicio por completo",
                onClick = onPowerOffAndExit,
                enabled = !state.busy,
                modifier = Modifier.fillMaxWidth(),
            )

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

        // 4. Multi-Format Export / EqualizerAPO Import
        NothingCard {
            NothingSectionLabel(
                text = "Importar / Exportar EQ",
                trailing = selectedExportFmt.uppercase(),
            )
            Spacer(Modifier.height(8.dp))
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .horizontalScroll(rememberScrollState()),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                listOf(
                    "apo" to "APO",
                    "easyeffects" to "EasyEffects",
                    "camilladsp" to "CamillaDSP",
                    "wavelet" to "Wavelet",
                ).forEach { (key, label) ->
                    NothingPillChip(
                        label = label,
                        selected = selectedExportFmt == key,
                        onClick = { selectedExportFmt = key },
                    )
                }
            }
            Spacer(Modifier.height(10.dp))
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                NothingOutlineButton(
                    text = "Importar APO",
                    onClick = { showImportDialog = true },
                    modifier = Modifier.weight(1f),
                )
                NothingOutlineButton(
                    text = "Copiar ${selectedExportFmt.uppercase()}",
                    onClick = {
                        val txt = onExportFormat(selectedExportFmt)
                        val cm = context.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
                        cm?.setPrimaryClip(ClipData.newPlainText("Trivium ${selectedExportFmt.uppercase()}", txt))
                        Toast.makeText(
                            context,
                            "Perfil ${selectedExportFmt.uppercase()} copiado al portapapeles",
                            Toast.LENGTH_SHORT,
                        ).show()
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
                    color = t.textSubtle,
                    fontFamily = ConsolasFontFamily,
                    fontSize = 11.sp,
                )
            } else {
                state.usbDiagnostics.forEach { u ->
                    Text(
                        text = "• ${u.productName} [${u.vidPid}] · HID=${u.hasHidInterface} · Perm=${u.hasPermission}" +
                            (u.recognizedAs?.let { " → $it" } ?: ""),
                        color = if (u.recognizedAs != null) t.emerald else t.textMuted,
                        fontFamily = ConsolasFontFamily,
                        fontSize = 11.sp,
                    )
                }
            }

            Spacer(Modifier.height(6.dp))
            if (state.audioOutputDiagnostics.isEmpty()) {
                Text(
                    text = "Salidas externas: solo altavoz interno.",
                    color = t.textSubtle,
                    fontFamily = ConsolasFontFamily,
                    fontSize = 11.sp,
                )
            } else {
                state.audioOutputDiagnostics.forEach { out ->
                    Text(
                        text = "• Audio: $out",
                        color = t.textMuted,
                        fontFamily = ConsolasFontFamily,
                        fontSize = 11.sp,
                    )
                }
            }

            Spacer(Modifier.height(8.dp))
            Text(
                text = "Protocolos USB DSP (v2.2.0): KTMicro 5B · Moondrop 8B · WalkPlay · FiiO 5/10B · Qudelix-5K · Topping · Fosi DS3 · JDS Element IV",
                color = t.textSubtle,
                fontFamily = ConsolasFontFamily,
                fontSize = 10.sp,
            )

            Spacer(Modifier.height(10.dp))
            NothingOutlineButton(
                text = "↻ Re-escanear hardware",
                onClick = onRefreshDiagnostics,
                modifier = Modifier.fillMaxWidth(),
            )
        }

        Text(
            text = "> ${state.statusMessage}",
            color = if (state.statusError) t.accent else t.textMuted,
            fontFamily = ConsolasFontFamily,
            fontSize = 11.sp,
            modifier = Modifier.padding(horizontal = 4.dp),
        )
        Spacer(Modifier.height(12.dp))
    }

    if (showImportDialog) {
        AlertDialog(
            onDismissRequest = { showImportDialog = false },
            containerColor = t.surface,
            titleContentColor = t.textPrimary,
            title = { Text("Importar ParametricEQ", fontFamily = ConsolasFontFamily) },
            text = {
                OutlinedTextField(
                    value = importText,
                    onValueChange = { importText = it },
                    placeholder = {
                        Text(
                            "Filter 1: ON PK Fc 105 Hz Gain 3.2 dB Q 0.707",
                            color = t.textSubtle,
                            fontFamily = ConsolasFontFamily,
                            fontSize = 11.sp,
                        )
                    },
                    minLines = 5,
                    maxLines = 8,
                    colors = OutlinedTextFieldDefaults.colors(
                        focusedBorderColor = t.accent,
                        unfocusedBorderColor = t.borderStrong,
                        focusedTextColor = t.textPrimary,
                        unfocusedTextColor = t.textPrimary,
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
                    Text("IMPORTAR", color = t.accent, fontFamily = ConsolasFontFamily, fontWeight = FontWeight.Bold)
                }
            },
            dismissButton = {
                TextButton(onClick = { showImportDialog = false }) {
                    Text("CANCELAR", color = t.textMuted, fontFamily = ConsolasFontFamily)
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
    val t = LocalTriviumTheme.current
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Column(modifier = Modifier.weight(1f).padding(end = 10.dp)) {
            Text(text = title, color = t.textPrimary, style = MaterialTheme.typography.bodyMedium, fontWeight = FontWeight.SemiBold)
            Text(text = subtitle, color = t.textSubtle, style = MaterialTheme.typography.labelSmall)
        }
        Switch(
            checked = checked,
            onCheckedChange = onCheckedChange,
            colors = SwitchDefaults.colors(
                checkedThumbColor = t.textOnAccent,
                checkedTrackColor = t.accent,
                uncheckedThumbColor = t.textMuted,
                uncheckedTrackColor = t.surfaceHigh,
                uncheckedBorderColor = t.borderStrong,
            ),
        )
    }
}
