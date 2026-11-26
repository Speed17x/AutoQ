package com.hector.moondropdsp

import android.Manifest
import android.content.Intent
import android.graphics.Color as AndroidColor
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.hector.moondropdsp.ui.MoondropUiState
import com.hector.moondropdsp.ui.MoondropViewModel
import com.hector.moondropdsp.ui.screens.AdvancedScreen
import com.hector.moondropdsp.ui.screens.AutoEqScreen
import com.hector.moondropdsp.ui.screens.EqScreen
import com.hector.moondropdsp.ui.theme.AppThemeId
import com.hector.moondropdsp.ui.theme.ConsolasFontFamily
import com.hector.moondropdsp.ui.theme.LocalAutoQTheme
import com.hector.moondropdsp.ui.theme.MoondropDSPTheme
import com.hector.moondropdsp.ui.theme.tokensFor

private enum class AppTab(val label: String) {
    EQ("EQ"),
    AUTOEQ("AUTOQ"),
    ADVANCED("AVANZADO"),
}

class MainActivity : ComponentActivity() {

    private val viewModel: MoondropViewModel by viewModels()

    private val permissionLauncher = registerForActivityResult(
        ActivityResultContracts.RequestMultiplePermissions(),
    ) {
        viewModel.onPermissionsChanged()
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        enableEdgeToEdge(
            statusBarStyle = SystemBarStyle.dark(AndroidColor.TRANSPARENT),
            navigationBarStyle = SystemBarStyle.dark(AndroidColor.TRANSPARENT),
        )

        requestOptionalRuntimePermissions()

        setContent {
            val state by viewModel.uiState.collectAsStateWithLifecycle()
            val tokens = tokensFor(state.themeId, state.accentId)
            androidx.compose.runtime.SideEffect {
                enableEdgeToEdge(
                    statusBarStyle = if (tokens.isDark) {
                        SystemBarStyle.dark(AndroidColor.TRANSPARENT)
                    } else {
                        SystemBarStyle.light(AndroidColor.TRANSPARENT, AndroidColor.TRANSPARENT)
                    },
                    navigationBarStyle = if (tokens.isDark) {
                        SystemBarStyle.dark(AndroidColor.TRANSPARENT)
                    } else {
                        SystemBarStyle.light(AndroidColor.TRANSPARENT, AndroidColor.TRANSPARENT)
                    },
                )
            }
            MoondropDSPTheme(themeId = state.themeId, accentId = state.accentId) {
                MoondropAppShell(
                    state = state,
                    viewModel = viewModel,
                    onRequestBtPermission = { requestOptionalRuntimePermissions() },
                    onPowerOffAndExit = {
                        viewModel.powerOffAndStopAll { finishAndRemoveTask() }
                    },
                )
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        viewModel.refreshDevices(readFromHardware = true)
    }

    private fun requestOptionalRuntimePermissions() {
        val perms = mutableListOf<String>()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            perms += Manifest.permission.BLUETOOTH_CONNECT
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            perms += Manifest.permission.POST_NOTIFICATIONS
        }
        if (perms.isNotEmpty()) {
            permissionLauncher.launch(perms.toTypedArray())
        }
    }
}

@Composable
private fun MoondropAppShell(
    state: MoondropUiState,
    viewModel: MoondropViewModel,
    onRequestBtPermission: () -> Unit,
    onPowerOffAndExit: () -> Unit,
) {
    var activeTab by rememberSaveable { mutableStateOf(AppTab.EQ) }
    val t = LocalAutoQTheme.current

    Scaffold(
        modifier = Modifier.fillMaxSize(),
        containerColor = t.background,
        contentWindowInsets = WindowInsets.safeDrawing,
        bottomBar = {
            NothingBottomTabBar(
                activeTab = activeTab,
                onSelectTab = { activeTab = it },
            )
        },
    ) { innerPadding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(t.background)
                .padding(innerPadding),
        ) {
            when (activeTab) {
                AppTab.EQ -> EqScreen(
                    state = state,
                    onSelectDevice = viewModel::selectDevice,
                    onRequestUsbPermission = viewModel::requestUsbPermission,
                    onRefresh = { viewModel.refreshDevices(readFromHardware = true) },
                    onToggleDsp = viewModel::toggleDspEnabled,
                    onToggleOverlayFr = viewModel::toggleOverlayFrMode,
                    onResetFlat = viewModel::resetFlat,
                    onSelectBand = viewModel::selectBand,
                    onUpdateBand = viewModel::updateBand,
                    onDragBand = viewModel::updateBandFromDrag,
                    onDragEnd = viewModel::onBandDragFinished,
                    onToggleLive = viewModel::toggleLivePreview,
                    onApply = { commit -> viewModel.applyCurrentBands(commit = commit) },
                    onSetPreampDb = viewModel::setPreampDb,
                    onSetAutoPreamp = viewModel::setAutoPreamp,
                    onSelectAbSlot = viewModel::selectAbSlot,
                    onCopyToOtherAbSlot = viewModel::copyCurrentToOtherAbSlot,
                    onPowerOffAndExit = onPowerOffAndExit,
                )

                AppTab.AUTOEQ -> AutoEqScreen(
                    state = state,
                    onLoadIndex = { force -> viewModel.ensureAutoEqIndexLoaded(forceRefresh = force) },
                    onSourceSearchChange = viewModel::updateSourceSearch,
                    onSelectSource = viewModel::selectSourceEntry,
                    onSetMode = viewModel::setAutoEqMode,
                    onSelectTarget = viewModel::selectTargetCurve,
                    onSearchQueryChange = viewModel::updateAutoEqSearch,
                    onSelectMatchEntry = viewModel::selectMatchHeadphone,
                    onSetMaxFreqFit = viewModel::setMaxFreqFit,
                    onComputeAndApply = { viewModel.computeAutoEq(applyImmediately = true) },
                    onNavigateToEq = { activeTab = AppTab.EQ },
                )

                AppTab.ADVANCED -> AdvancedScreen(
                    state = state,
                    onSelectTheme = viewModel::selectTheme,
                    onSelectAccent = viewModel::selectAccent,
                    onApplyPreset = viewModel::applyPreset,
                    onSavePreset = viewModel::saveCustomPreset,
                    onDeletePreset = viewModel::deleteCustomPreset,
                    onExportApo = viewModel::exportParametricEqText,
                    onExportFormat = viewModel::exportEqInFormat,
                    onImportApo = viewModel::importParametricEqText,
                    onSetBtGlobalEq = viewModel::setBtGlobalEqEnabled,
                    onSetBtSessionZero = viewModel::setBtSessionZeroFallback,
                    onRequestBluetoothPermission = onRequestBtPermission,
                    onBtSppRead = viewModel::btSppReadEq,
                    onBtSppWrite = viewModel::btSppWriteEq,
                    onBtSppFirmwarePreset = viewModel::btSppSelectFirmwarePreset,
                    onRefreshDiagnostics = { viewModel.refreshDevices(readFromHardware = false) },
                    onPowerOffAndExit = onPowerOffAndExit,
                )
            }
        }
    }
}

@Composable
private fun NothingBottomTabBar(
    activeTab: AppTab,
    onSelectTab: (AppTab) -> Unit,
) {
    val t = LocalAutoQTheme.current
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .background(t.background)
            .navigationBarsPadding()
            .padding(horizontal = 20.dp, vertical = 10.dp),
        contentAlignment = Alignment.Center,
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clip(CircleShape)
                .background(t.surface)
                .border(1.dp, t.border, CircleShape)
                .padding(5.dp),
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            AppTab.entries.forEach { tab ->
                val selected = tab == activeTab
                Box(
                    modifier = Modifier
                        .weight(1f)
                        .height(40.dp)
                        .clip(CircleShape)
                        .background(if (selected) t.accent else t.surfaceHigh)
                        .clickable { onSelectTab(tab) },
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        text = tab.label,
                        color = if (selected) t.textOnAccent else t.textMuted,
                        fontFamily = ConsolasFontFamily,
                        fontWeight = FontWeight.Bold,
                        fontSize = 11.sp,
                        letterSpacing = 0.8.sp,
                    )
                }
            }
        }
    }
}

@Preview(showBackground = true, backgroundColor = 0xFF0E1116)
@Composable
fun EqScreenPreview() {
    MoondropDSPTheme {
        val t = LocalAutoQTheme.current
        Column(
            modifier = Modifier
                .fillMaxSize()
                .background(t.background)
                .statusBarsPadding(),
        ) {
            EqScreen(
                state = MoondropUiState(),
                onSelectDevice = {},
                onRequestUsbPermission = {},
                onRefresh = {},
                onToggleDsp = {},
                onToggleOverlayFr = {},
                onResetFlat = {},
                onSelectBand = {},
                onUpdateBand = { _, _ -> },
                onDragBand = { _, _, _ -> },
                onDragEnd = {},
                onToggleLive = {},
                onApply = {},
                onPowerOffAndExit = {},
            )
            Spacer(Modifier.height(4.dp))
        }
    }
}
