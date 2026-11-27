package com.speed17x.trivium.widget

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.widget.RemoteViews
import androidx.compose.ui.graphics.toArgb
import com.speed17x.trivium.MainActivity
import com.speed17x.trivium.R
import com.speed17x.trivium.data.AppPreferencesStore
import com.speed17x.trivium.hardware.BluetoothMoondropTransport
import com.speed17x.trivium.hardware.DeviceKind
import com.speed17x.trivium.hardware.GlobalEqService
import com.speed17x.trivium.hardware.UsbMoondropTransport
import com.speed17x.trivium.ui.theme.AppAccentId
import com.speed17x.trivium.ui.theme.AppThemeId
import com.speed17x.trivium.ui.theme.tokensFor
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch

class NothingEqWidgetProvider : AppWidgetProvider() {

    companion object {
        const val ACTION_TOGGLE_EQ = "com.speed17x.trivium.widget.ACTION_TOGGLE_EQ"
        const val ACTION_PREV_PRESET = "com.speed17x.trivium.widget.ACTION_PREV_PRESET"
        const val ACTION_NEXT_PRESET = "com.speed17x.trivium.widget.ACTION_NEXT_PRESET"
        const val ACTION_REFRESH = "com.speed17x.trivium.widget.ACTION_REFRESH"

        private val widgetScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

        fun updateAllWidgets(context: Context) {
            val mgr = AppWidgetManager.getInstance(context)
            val ids = mgr.getAppWidgetIds(ComponentName(context, NothingEqWidgetProvider::class.java))
            if (ids.isEmpty()) return
            val views = buildRemoteViews(context)
            for (id in ids) {
                mgr.updateAppWidget(id, views)
            }
        }

        fun buildRemoteViews(context: Context): RemoteViews {
            val store = AppPreferencesStore(context)
            val snap = store.loadSnapshot()
            val themeId = AppThemeId.fromKey(snap.themeId)
            val accentId = AppAccentId.fromKey(snap.accentId)
            val tokens = tokensFor(themeId, accentId)
            val views = RemoteViews(context.packageName, R.layout.widget_nothing_eq)

            val rootBgRes = when (themeId) {
                AppThemeId.PURE_HYPER -> R.drawable.widget_bg_default
                AppThemeId.PURE_LIGHT -> R.drawable.widget_bg_light
                AppThemeId.NOTHING_OS -> R.drawable.widget_bg_nothing
            }
            val graphBgRes = when (themeId) {
                AppThemeId.PURE_HYPER -> R.drawable.widget_graph_frame_default
                AppThemeId.PURE_LIGHT -> R.drawable.widget_graph_frame_light
                AppThemeId.NOTHING_OS -> R.drawable.widget_graph_frame
            }
            val pillBgRes = when (themeId) {
                AppThemeId.PURE_HYPER -> R.drawable.widget_pill_default
                AppThemeId.PURE_LIGHT -> R.drawable.widget_pill_light
                AppThemeId.NOTHING_OS -> R.drawable.widget_pill_dark
            }

            views.setInt(R.id.widget_root, "setBackgroundResource", rootBgRes)
            views.setInt(R.id.widget_graph_container, "setBackgroundResource", graphBgRes)
            views.setInt(R.id.widget_btn_prev, "setBackgroundResource", pillBgRes)
            views.setInt(R.id.widget_preset_label, "setBackgroundResource", pillBgRes)
            views.setInt(R.id.widget_btn_next, "setBackgroundResource", pillBgRes)
            views.setInt(R.id.widget_btn_open, "setBackgroundResource", pillBgRes)

            val primaryArgb = tokens.textPrimary.toArgb()
            val mutedArgb = tokens.textMuted.toArgb()
            val accentArgb = tokens.accent.toArgb()
            val onAccentArgb = tokens.textOnAccent.toArgb()

            val isActive = snap.connected && snap.dspEnabled
            views.setImageViewBitmap(
                R.id.widget_status_dot,
                NothingDotMatrixRenderer.renderStatusDot(if (isActive) accentArgb else tokens.textSubtle.toArgb()),
            )

            val titleText = snap.shortWidgetName.ifBlank { "TRIVIUM" }
            val titleColor = if (snap.connected) primaryArgb else mutedArgb
            val titleBmp = if (tokens.useDotMatrixHeaders) {
                NothingDotMatrixRenderer.renderDotMatrixTitle(text = titleText, color = titleColor)
            } else {
                NothingDotMatrixRenderer.renderCleanTitle(text = titleText, color = titleColor)
            }
            views.setImageViewBitmap(R.id.widget_dot_title, titleBmp)

            views.setTextViewText(R.id.widget_transport_badge, snap.transportBadge)
            views.setTextColor(R.id.widget_transport_badge, mutedArgb)

            views.setTextViewText(R.id.widget_btn_power, if (snap.dspEnabled) "ON" else "OFF")
            val powerFill = if (snap.dspEnabled) accentArgb else tokens.surfaceHigh.toArgb()
            val powerStroke = if (snap.dspEnabled) accentArgb else tokens.border.toArgb()
            views.setImageViewBitmap(
                R.id.widget_btn_power_bg,
                NothingDotMatrixRenderer.renderPowerPill(powerFill, powerStroke),
            )
            views.setTextColor(
                R.id.widget_btn_power,
                if (snap.dspEnabled) onAccentArgb else mutedArgb,
            )

            val curveBmp = NothingDotMatrixRenderer.renderCurveBitmap(
                bands = snap.bands,
                dspEnabled = snap.dspEnabled,
                overlayFrMode = snap.overlayFrMode,
                stockFr = snap.stockFrCurve,
                targetFr = snap.targetFrCurve,
                tokens = tokens,
            )
            views.setImageViewBitmap(R.id.widget_curve_image, curveBmp)

            views.setTextViewText(
                R.id.widget_preset_label,
                snap.activePresetName.uppercase(),
            )
            views.setTextColor(R.id.widget_btn_prev, primaryArgb)
            views.setTextColor(R.id.widget_preset_label, primaryArgb)
            views.setTextColor(R.id.widget_btn_next, primaryArgb)
            views.setTextColor(R.id.widget_btn_open, accentArgb)

            // Click handlers
            val openAppPi = PendingIntent.getActivity(
                context,
                100,
                Intent(context, MainActivity::class.java).apply {
                    flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP
                },
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )
            views.setOnClickPendingIntent(R.id.widget_graph_container, openAppPi)
            views.setOnClickPendingIntent(R.id.widget_btn_open, openAppPi)
            views.setOnClickPendingIntent(R.id.widget_dot_title, openAppPi)

            val powerPi = actionPendingIntent(context, ACTION_TOGGLE_EQ, 101)
            views.setOnClickPendingIntent(R.id.widget_btn_power_wrap, powerPi)
            views.setOnClickPendingIntent(R.id.widget_btn_power, powerPi)
            views.setOnClickPendingIntent(R.id.widget_btn_prev, actionPendingIntent(context, ACTION_PREV_PRESET, 102))
            views.setOnClickPendingIntent(R.id.widget_btn_next, actionPendingIntent(context, ACTION_NEXT_PRESET, 103))
            views.setOnClickPendingIntent(R.id.widget_preset_label, actionPendingIntent(context, ACTION_REFRESH, 104))

            return views
        }

        private fun actionPendingIntent(context: Context, action: String, reqCode: Int): PendingIntent {
            val intent = Intent(context, NothingEqWidgetProvider::class.java).setAction(action)
            return PendingIntent.getBroadcast(
                context,
                reqCode,
                intent,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )
        }
    }

    override fun onUpdate(context: Context, appWidgetManager: AppWidgetManager, appWidgetIds: IntArray) {
        val views = buildRemoteViews(context)
        for (id in appWidgetIds) {
            appWidgetManager.updateAppWidget(id, views)
        }
    }

    override fun onReceive(context: Context, intent: Intent) {
        super.onReceive(context, intent)
        val action = intent.action ?: return
        if (action !in setOf(ACTION_TOGGLE_EQ, ACTION_PREV_PRESET, ACTION_NEXT_PRESET, ACTION_REFRESH)) return

        val pendingResult = goAsync()
        widgetScope.launch {
            try {
                handleWidgetAction(context.applicationContext, action)
            } finally {
                updateAllWidgets(context.applicationContext)
                pendingResult.finish()
            }
        }
    }

    private fun handleWidgetAction(context: Context, action: String) {
        val store = AppPreferencesStore(context)
        val usb = UsbMoondropTransport(context)
        val bt = BluetoothMoondropTransport(context)

        val usbDevices = usb.scanDevices()
        val extDevices = bt.scanConnectedExternalAudio(usbDevices)
        val activeDev = usbDevices.firstOrNull() ?: extDevices.firstOrNull()

        var snap = store.updateSnapshot { cur ->
            if (activeDev != null) {
                cur.copy(
                    deviceName = activeDev.displayName,
                    shortWidgetName = activeDev.shortWidgetName,
                    transportBadge = activeDev.transportBadge,
                    connected = true,
                )
            } else {
                cur.copy(
                    deviceName = "Sin dispositivo",
                    shortWidgetName = "TRIVIUM",
                    transportBadge = "LISTO",
                    connected = false,
                )
            }
        }

        when (action) {
            ACTION_TOGGLE_EQ -> {
                snap = store.updateSnapshot { it.copy(dspEnabled = !it.dspEnabled) }
                pushToActiveDevice(context, usb, activeDev, snap.bands, snap.dspEnabled)
            }

            ACTION_PREV_PRESET, ACTION_NEXT_PRESET -> {
                val presets = store.allPresets(snap)
                if (presets.isNotEmpty()) {
                    val curIdx = presets.indexOfFirst { it.name == snap.activePresetName }
                    val nextIdx = if (action == ACTION_NEXT_PRESET) {
                        if (curIdx < 0) 0 else (curIdx + 1) % presets.size
                    } else {
                        if (curIdx <= 0) presets.lastIndex else curIdx - 1
                    }
                    val chosen = presets[nextIdx]
                    snap = store.updateSnapshot {
                        it.copy(
                            activePresetName = chosen.name,
                            bands = chosen.bands,
                            dspEnabled = true,
                            targetFrCurve = emptyList(),
                            targetLabel = "",
                        )
                    }
                    pushToActiveDevice(context, usb, activeDev, snap.bands, true)
                }
            }

            ACTION_REFRESH -> {
                if (activeDev != null && activeDev.kind == DeviceKind.USB_HID && activeDev.hasUsbPermission) {
                    try {
                        val (enabled, bands) = usb.readEq(activeDev)
                        store.updateSnapshot { it.copy(dspEnabled = enabled, bands = bands) }
                    } catch (_: Exception) {
                    }
                }
            }
        }
    }

    private fun pushToActiveDevice(
        context: Context,
        usb: UsbMoondropTransport,
        activeDev: com.speed17x.trivium.hardware.MoondropDevice?,
        bands: List<com.speed17x.trivium.dsp.FilterBand>,
        enableEq: Boolean,
    ) {
        if (!enableEq) {
            GlobalEqService.stop(context)
        }
        if (activeDev == null) {
            if (enableEq) {
                GlobalEqService.startOrUpdate(context)
            }
            return
        }
        when (activeDev.kind) {
            DeviceKind.USB_HID -> {
                if (activeDev.hasUsbPermission) {
                    try {
                        usb.writeEq(activeDev, bands, commit = false, enableEq = enableEq)
                    } catch (_: Exception) {
                    }
                }
            }

            DeviceKind.BT_AUDIO, DeviceKind.USB_AUDIO, DeviceKind.WIRED_AUDIO, DeviceKind.BLE_GATT -> {
                if (enableEq) {
                    GlobalEqService.startOrUpdate(context)
                } else {
                    GlobalEqService.stop(context)
                }
            }
        }
    }
}
