package com.hector.moondropdsp.widget

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.widget.RemoteViews
import com.hector.moondropdsp.MainActivity
import com.hector.moondropdsp.R
import com.hector.moondropdsp.data.AppPreferencesStore
import com.hector.moondropdsp.hardware.BluetoothMoondropTransport
import com.hector.moondropdsp.hardware.DeviceKind
import com.hector.moondropdsp.hardware.GlobalEqService
import com.hector.moondropdsp.hardware.UsbMoondropTransport
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch

class NothingEqWidgetProvider : AppWidgetProvider() {

    companion object {
        const val ACTION_TOGGLE_EQ = "com.hector.moondropdsp.widget.ACTION_TOGGLE_EQ"
        const val ACTION_PREV_PRESET = "com.hector.moondropdsp.widget.ACTION_PREV_PRESET"
        const val ACTION_NEXT_PRESET = "com.hector.moondropdsp.widget.ACTION_NEXT_PRESET"
        const val ACTION_REFRESH = "com.hector.moondropdsp.widget.ACTION_REFRESH"

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
            val views = RemoteViews(context.packageName, R.layout.widget_nothing_eq)

            val isActive = snap.connected && snap.dspEnabled
            views.setImageViewResource(
                R.id.widget_status_dot,
                if (isActive) R.drawable.widget_dot_red else R.drawable.widget_dot_dim,
            )

            val titleBmp = NothingDotMatrixRenderer.renderDotMatrixTitle(
                text = snap.shortWidgetName.ifBlank { "AUTOQ" },
                color = if (snap.connected) 0xFFF5F5F5.toInt() else 0xFF9EA2AC.toInt(),
            )
            views.setImageViewBitmap(R.id.widget_dot_title, titleBmp)

            views.setTextViewText(R.id.widget_transport_badge, snap.transportBadge)

            views.setTextViewText(R.id.widget_btn_power, if (snap.dspEnabled) "ON" else "OFF")
            views.setInt(
                R.id.widget_btn_power,
                "setBackgroundResource",
                if (snap.dspEnabled) R.drawable.widget_pill_red else R.drawable.widget_pill_dark,
            )
            views.setTextColor(
                R.id.widget_btn_power,
                if (snap.dspEnabled) 0xFFFFFFFF.toInt() else 0xFF8E929B.toInt(),
            )

            val curveBmp = NothingDotMatrixRenderer.renderCurveBitmap(
                bands = snap.bands,
                dspEnabled = snap.dspEnabled,
                overlayFrMode = snap.overlayFrMode,
                stockFr = snap.stockFrCurve,
                targetFr = snap.targetFrCurve,
            )
            views.setImageViewBitmap(R.id.widget_curve_image, curveBmp)

            views.setTextViewText(
                R.id.widget_preset_label,
                snap.activePresetName.uppercase(),
            )

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

            views.setOnClickPendingIntent(R.id.widget_btn_power, actionPendingIntent(context, ACTION_TOGGLE_EQ, 101))
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
                    shortWidgetName = "AUTOQ",
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
        activeDev: com.hector.moondropdsp.hardware.MoondropDevice?,
        bands: List<com.hector.moondropdsp.dsp.FilterBand>,
        enableEq: Boolean,
    ) {
        if (activeDev == null) {
            GlobalEqService.startOrUpdate(context)
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

            DeviceKind.BT_AUDIO, DeviceKind.USB_AUDIO, DeviceKind.WIRED_AUDIO -> {
                GlobalEqService.startOrUpdate(context)
            }
        }
    }
}
