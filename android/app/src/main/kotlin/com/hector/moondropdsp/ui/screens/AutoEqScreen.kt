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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.hector.moondropdsp.data.AutoEqEntry
import com.hector.moondropdsp.data.AutoEqParser
import com.hector.moondropdsp.ui.AutoEqMode
import com.hector.moondropdsp.ui.MoondropUiState
import com.hector.moondropdsp.ui.components.DotMatrixText
import com.hector.moondropdsp.ui.components.NothingCard
import com.hector.moondropdsp.ui.components.NothingOutlineButton
import com.hector.moondropdsp.ui.components.NothingPillChip
import com.hector.moondropdsp.ui.components.NothingPrimaryButton
import com.hector.moondropdsp.ui.components.NothingSectionLabel
import com.hector.moondropdsp.ui.theme.NothingAmber
import com.hector.moondropdsp.ui.theme.NothingBorder
import com.hector.moondropdsp.ui.theme.NothingBorderStrong
import com.hector.moondropdsp.ui.theme.NothingEmerald
import com.hector.moondropdsp.ui.theme.NothingGraphBg
import com.hector.moondropdsp.ui.theme.NothingMuted
import com.hector.moondropdsp.ui.theme.NothingRed
import com.hector.moondropdsp.ui.theme.NothingSubtle
import com.hector.moondropdsp.ui.theme.NothingSurfaceHigh
import com.hector.moondropdsp.ui.theme.NothingWhite
import java.util.Locale

@Composable
fun AutoEqScreen(
    state: MoondropUiState,
    onLoadIndex: (forceRefresh: Boolean) -> Unit,
    onSourceSearchChange: (String) -> Unit,
    onSelectSource: (AutoEqEntry) -> Unit,
    onSetMode: (AutoEqMode) -> Unit,
    onSelectTarget: (Int) -> Unit,
    onSearchQueryChange: (String) -> Unit,
    onSelectMatchEntry: (AutoEqEntry) -> Unit,
    onSetMaxFreqFit: (Double) -> Unit,
    onComputeAndApply: () -> Unit,
    onNavigateToEq: () -> Unit,
    modifier: Modifier = Modifier,
) {
    LaunchedEffect(Unit) {
        onLoadIndex(false)
    }

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        // Header Card
        NothingCard {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    DotMatrixText(text = "AUTOQ DB", dotSize = 2.4.dp, dotGap = 1.0.dp)
                    Spacer(Modifier.height(4.dp))
                    Text(
                        text = if (state.autoEqTotalCount > 0) {
                            "${state.autoEqTotalCount} CURVAS · ${state.autoEqActiveSources} FUENTES (AUTOEQ + SQUIGLINK)"
                        } else if (state.autoEqLoadingIndex) {
                            "SINCRONIZANDO AUTOEQ + SQUIGLINK..."
                        } else {
                            "BASE UNIVERSAL AUTOEQ + SQUIGLINK (IEM · TWS · OVER-EAR)"
                        },
                        color = NothingMuted,
                        style = MaterialTheme.typography.labelSmall,
                    )
                }
                NothingOutlineButton(
                    text = if (state.autoEqLoadingIndex) "..." else "↻ Sync",
                    onClick = { onLoadIndex(true) },
                    enabled = !state.autoEqLoadingIndex,
                )
            }
        }

        // Universal Source Headphone Picker (ANY brand / model)
        NothingCard {
            NothingSectionLabel(
                text = "Tu audífono actual (origen)",
                trailing = state.activeDevice?.shortWidgetName ?: "Cualquier marca",
            )
            Spacer(Modifier.height(8.dp))

            state.selectedSourceEntry?.let { sel ->
                Text(
                    text = "SELECCIONADO: ${sel.label}",
                    color = NothingEmerald,
                    style = MaterialTheme.typography.labelSmall,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.padding(bottom = 8.dp),
                )
            }

            OutlinedTextField(
                value = state.sourceSearchQuery,
                onValueChange = onSourceSearchChange,
                placeholder = {
                    Text(
                        "Buscar tu audífono (Sony, Huawei, Nothing, Moondrop, KZ...)",
                        color = NothingSubtle,
                        style = MaterialTheme.typography.bodyMedium,
                    )
                },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
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

            Spacer(Modifier.height(8.dp))

            // Quick brand filter chips
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .horizontalScroll(rememberScrollState()),
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                listOf(
                    "" to "Todos",
                    "7Hz" to "7Hz",
                    "Moondrop" to "Moondrop",
                    "Sony" to "Sony",
                    "Huawei" to "Huawei",
                    "Nothing" to "Nothing",
                    "Apple" to "Apple",
                    "Samsung" to "Samsung",
                    "Truthear" to "Truthear",
                    "KZ" to "KZ",
                    "Sennheiser" to "Sennheiser",
                ).forEach { (q, lbl) ->
                    NothingPillChip(
                        label = lbl,
                        selected = state.sourceSearchQuery.equals(q, ignoreCase = true),
                        onClick = { onSourceSearchChange(q) },
                        accentRed = false,
                    )
                }
            }

            Spacer(Modifier.height(8.dp))

            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(175.dp)
                    .clip(RoundedCornerShape(14.dp))
                    .background(NothingGraphBg)
                    .border(1.dp, NothingBorder, RoundedCornerShape(14.dp)),
            ) {
                LazyColumn(modifier = Modifier.fillMaxSize()) {
                    items(state.sourceSearchResults, key = { "src:${it.uniqueKey}" }) { entry ->
                        val isSel = state.selectedSourceEntry?.uniqueKey == entry.uniqueKey
                        Column(
                            modifier = Modifier
                                .fillMaxWidth()
                                .background(if (isSel) NothingSurfaceHigh else NothingGraphBg)
                                .clickable { onSelectSource(entry) }
                                .padding(horizontal = 12.dp, vertical = 8.dp),
                        ) {
                            Text(
                                text = entry.name,
                                color = if (isSel) NothingRed else NothingWhite,
                                style = MaterialTheme.typography.bodyMedium,
                                fontWeight = if (isSel) FontWeight.Bold else FontWeight.Normal,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                            Text(
                                text = "${entry.sourceRig} · ${entry.formFactor}",
                                color = NothingSubtle,
                                style = MaterialTheme.typography.labelSmall,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                    }
                }
            }
        }

        // Mode Switch: Curva Target vs Imitar Audífono
        NothingCard {
            NothingSectionLabel(text = "Objetivo de ecualización")
            Spacer(Modifier.height(10.dp))

            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                NothingPillChip(
                    label = "CURVA TARGET",
                    selected = state.autoEqMode == AutoEqMode.TARGET_CURVE,
                    onClick = { onSetMode(AutoEqMode.TARGET_CURVE) },
                    accentRed = true,
                    modifier = Modifier.weight(1f),
                )
                NothingPillChip(
                    label = "IMITAR AUDÍFONO",
                    selected = state.autoEqMode == AutoEqMode.MATCH_HEADPHONE,
                    onClick = { onSetMode(AutoEqMode.MATCH_HEADPHONE) },
                    accentRed = true,
                    modifier = Modifier.weight(1f),
                )
            }

            Spacer(Modifier.height(12.dp))

            if (state.autoEqMode == AutoEqMode.TARGET_CURVE) {
                Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    AutoEqParser.CURATED_TARGETS.forEachIndexed { idx, target ->
                        val selected = idx == state.selectedTargetIdx
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clip(RoundedCornerShape(14.dp))
                                .background(if (selected) NothingSurfaceHigh else NothingGraphBg)
                                .border(1.dp, if (selected) NothingRed else NothingBorder, RoundedCornerShape(14.dp))
                                .clickable { onSelectTarget(idx) }
                                .padding(horizontal = 12.dp, vertical = 10.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.SpaceBetween,
                        ) {
                            Text(
                                text = target.title,
                                color = if (selected) NothingWhite else NothingMuted,
                                style = MaterialTheme.typography.bodyMedium,
                                fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal,
                                modifier = Modifier.weight(1f),
                            )
                            if (selected) {
                                Text(
                                    text = "●",
                                    color = NothingRed,
                                    fontSize = 12.sp,
                                )
                            }
                        }
                    }
                }
            } else {
                OutlinedTextField(
                    value = state.searchQuery,
                    onValueChange = onSearchQueryChange,
                    placeholder = {
                        Text(
                            "Buscar objetivo (Truthear Hexa, Blessing 3, HD 600, AirPods Pro...)",
                            color = NothingSubtle,
                            style = MaterialTheme.typography.bodyMedium,
                        )
                    },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
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

                Spacer(Modifier.height(8.dp))

                state.selectedMatchEntry?.let { sel ->
                    Text(
                        text = "OBJETIVO: ${sel.label}",
                        color = NothingEmerald,
                        style = MaterialTheme.typography.labelSmall,
                        modifier = Modifier.padding(bottom = 6.dp),
                    )
                }

                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(190.dp)
                        .clip(RoundedCornerShape(14.dp))
                        .background(NothingGraphBg)
                        .border(1.dp, NothingBorder, RoundedCornerShape(14.dp)),
                ) {
                    LazyColumn(modifier = Modifier.fillMaxSize()) {
                        items(state.searchResults, key = { "tgt:${it.uniqueKey}" }) { entry ->
                            val isSel = state.selectedMatchEntry?.uniqueKey == entry.uniqueKey
                            Column(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .background(if (isSel) NothingSurfaceHigh else NothingGraphBg)
                                    .clickable { onSelectMatchEntry(entry) }
                                    .padding(horizontal = 12.dp, vertical = 8.dp),
                            ) {
                                Text(
                                    text = entry.name,
                                    color = if (isSel) NothingRed else NothingWhite,
                                    style = MaterialTheme.typography.bodyMedium,
                                    fontWeight = if (isSel) FontWeight.Bold else FontWeight.Normal,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                                Text(
                                    text = "${entry.sourceRig} · ${entry.formFactor}",
                                    color = NothingSubtle,
                                    style = MaterialTheme.typography.labelSmall,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            }
                        }
                    }
                }
            }

            Spacer(Modifier.height(12.dp))
            NothingSectionLabel(text = "Límite superior de ajuste", trailing = "${(state.maxFreqFitHz / 1000.0).toInt()} kHz")
            Spacer(Modifier.height(6.dp))
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .horizontalScroll(rememberScrollState()),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                listOf(6000.0 to "6 kHz", 8000.0 to "8 kHz", 10000.0 to "10 kHz", 14000.0 to "14 kHz").forEach { (hz, lbl) ->
                    NothingPillChip(
                        label = lbl,
                        selected = state.maxFreqFitHz == hz,
                        onClick = { onSetMaxFreqFit(hz) },
                        accentRed = false,
                    )
                }
            }

            Spacer(Modifier.height(14.dp))
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(10.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                NothingPrimaryButton(
                    text = if (state.autoEqComputing) "Calculando..." else "Calcular y Aplicar AutoQ",
                    onClick = onComputeAndApply,
                    enabled = !state.autoEqComputing && !state.autoEqLoadingIndex && state.selectedSourceEntry != null,
                    modifier = Modifier.weight(1f),
                )
                if (state.autoEqComputing) {
                    CircularProgressIndicator(
                        color = NothingRed,
                        strokeWidth = 2.dp,
                        modifier = Modifier.height(24.dp),
                    )
                }
            }
        }

        // Last Result Summary
        state.lastAutoEqResult?.let { res ->
            NothingCard(borderColor = NothingBorderStrong) {
                NothingSectionLabel(
                    text = "Último ajuste aplicado",
                    trailing = "RMSE ${String.format(Locale.US, "%.2f", res.rmseDb)} dB",
                )
                Spacer(Modifier.height(6.dp))
                Text(
                    text = res.targetLabel,
                    color = NothingWhite,
                    style = MaterialTheme.typography.titleMedium,
                )
                res.warning?.let { warn ->
                    Spacer(Modifier.height(6.dp))
                    Text(
                        text = warn,
                        color = NothingAmber,
                        style = MaterialTheme.typography.bodyMedium,
                    )
                }
                Spacer(Modifier.height(10.dp))
                NothingOutlineButton(
                    text = "Ver curvas superpuestas en EQ →",
                    onClick = onNavigateToEq,
                    modifier = Modifier.fillMaxWidth(),
                    accentText = NothingWhite,
                )
            }
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
}
