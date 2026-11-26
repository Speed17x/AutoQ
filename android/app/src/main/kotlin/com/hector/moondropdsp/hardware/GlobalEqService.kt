package com.hector.moondropdsp.hardware

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.ServiceInfo
import android.media.audiofx.AudioEffect
import android.media.audiofx.DynamicsProcessing
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import com.hector.moondropdsp.MainActivity
import com.hector.moondropdsp.data.AppPreferencesStore
import com.hector.moondropdsp.dsp.BiquadMath
import com.hector.moondropdsp.dsp.FilterBand
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import java.lang.ref.WeakReference
import kotlin.math.max
import kotlin.math.sqrt

/**
 * Foreground service running Android's native DynamicsProcessing 32-band EQ
 * sampled continuously from our 10-band parametric RBJ biquad curve for any Bluetooth,
 * generic USB-C, or 3.5mm wired headphones.
 */
class GlobalEqService : Service() {

    companion object {
        const val ACTION_APPLY = "com.hector.moondropdsp.eq.ACTION_APPLY"
        const val ACTION_STOP = "com.hector.moondropdsp.eq.ACTION_STOP"
        const val ACTION_POWER_OFF = "com.hector.moondropdsp.eq.ACTION_POWER_OFF"
        private const val CHANNEL_ID = "moondrop_eq_service"
        private const val NOTIF_ID = 4102
        private const val EQ_BAND_COUNT = 32

        @Volatile
        private var activeInstanceRef: WeakReference<GlobalEqService>? = null

        private val _running = MutableStateFlow(false)
        val running: StateFlow<Boolean> = _running.asStateFlow()

        private val _attachedSessionCount = MutableStateFlow(0)
        val attachedSessionCount: StateFlow<Int> = _attachedSessionCount.asStateFlow()

        private val _lastError = MutableStateFlow<String?>(null)
        val lastError: StateFlow<String?> = _lastError.asStateFlow()

        /**
         * Fast path: if service is already alive in-process, apply DSP gains directly without
         * spamming startForegroundService() Binder calls during live slider dragging.
         */
        fun startOrUpdate(context: Context, liveOnlyIfRunning: Boolean = false) {
            val runningService = activeInstanceRef?.get()
            if (runningService != null) {
                runningService.refreshFromStore(updateNotification = !liveOnlyIfRunning)
                return
            }
            val intent = Intent(context, GlobalEqService::class.java).setAction(ACTION_APPLY)
            try {
                ContextCompat.startForegroundService(context, intent)
            } catch (e: Exception) {
                _lastError.value = "No se pudo iniciar el servicio EQ: ${e.message}"
            }
        }

        fun stop(context: Context) {
            activeInstanceRef?.get()?.shutdownServiceCompletely()
            val intent = Intent(context, GlobalEqService::class.java).setAction(ACTION_STOP)
            context.stopService(intent)
        }
    }

    private lateinit var store: AppPreferencesStore
    private val processorsLock = Any()
    private val eqCenterFreqs = BiquadMath.logSpace(28.0, 18000.0, EQ_BAND_COUNT)
    private val eqCutoffFreqs = FloatArray(EQ_BAND_COUNT) { i ->
        if (i == EQ_BAND_COUNT - 1) 20000f
        else sqrt(eqCenterFreqs[i] * eqCenterFreqs[i + 1]).toFloat()
    }

    private val processors = LinkedHashMap<Int, DynamicsProcessing>()

    private val sessionReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context?, intent: Intent?) {
            val action = intent?.action ?: return
            val sessionId = intent.getIntExtra(AudioEffect.EXTRA_AUDIO_SESSION, -1)
            if (sessionId <= 0) return
            when (action) {
                AudioEffect.ACTION_OPEN_AUDIO_EFFECT_CONTROL_SESSION -> {
                    attachSession(sessionId)
                    refreshFromStore(updateNotification = false)
                }
                AudioEffect.ACTION_CLOSE_AUDIO_EFFECT_CONTROL_SESSION -> {
                    detachSession(sessionId)
                }
            }
        }
    }

    override fun onCreate() {
        super.onCreate()
        store = AppPreferencesStore(this)
        ensureNotificationChannel()

        val filter = IntentFilter().apply {
            addAction(AudioEffect.ACTION_OPEN_AUDIO_EFFECT_CONTROL_SESSION)
            addAction(AudioEffect.ACTION_CLOSE_AUDIO_EFFECT_CONTROL_SESSION)
        }
        ContextCompat.registerReceiver(
            this,
            sessionReceiver,
            filter,
            ContextCompat.RECEIVER_EXPORTED,
        )
        activeInstanceRef = WeakReference(this)
        _running.value = true
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP || intent?.action == ACTION_POWER_OFF) {
            if (intent.action == ACTION_POWER_OFF) {
                store.updateSnapshot { it.copy(dspEnabled = false) }
                com.hector.moondropdsp.widget.NothingEqWidgetProvider.updateAllWidgets(this)
            }
            shutdownServiceCompletely()
            return START_NOT_STICKY
        }

        val state = store.loadSnapshot()
        if (!state.dspEnabled || !state.btGlobalEqEnabled) {
            shutdownServiceCompletely()
            return START_NOT_STICKY
        }

        val notif = buildNotification(state.deviceName, state.activePresetName, state.dspEnabled)
        try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                ServiceCompat.startForeground(
                    this,
                    NOTIF_ID,
                    notif,
                    ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK,
                )
            } else {
                startForeground(NOTIF_ID, notif)
            }
        } catch (e: Exception) {
            _lastError.value = "Error FGS: ${e.message}"
        }

        refreshFromStore(updateNotification = false)
        return START_STICKY
    }

    private fun shutdownServiceCompletely() {
        releaseAllProcessors()
        ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
        _running.value = false
        stopSelf()
    }

    private fun releaseAllProcessors() {
        synchronized(processorsLock) {
            processors.values.forEach { dp ->
                try {
                    dp.enabled = false
                    dp.release()
                } catch (_: Exception) {
                }
            }
            processors.clear()
            _attachedSessionCount.value = 0
        }
    }

    override fun onDestroy() {
        activeInstanceRef = null
        try {
            unregisterReceiver(sessionReceiver)
        } catch (_: Exception) {
        }
        releaseAllProcessors()
        _running.value = false
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    private fun attachSession(sessionId: Int) = synchronized(processorsLock) {
        if (processors.containsKey(sessionId)) return@synchronized
        try {
            val cfg = DynamicsProcessing.Config.Builder(
                DynamicsProcessing.VARIANT_FAVOR_FREQUENCY_RESOLUTION,
                2,
                false, 0,
                false, 0,
                true, EQ_BAND_COUNT,
                true,
            ).build()

            val dp = DynamicsProcessing(0, sessionId, cfg)
            val limiter = DynamicsProcessing.Limiter(
                true,
                true,
                0,
                1.0f,
                60.0f,
                10.0f,
                -0.3f,
                0.0f,
            )
            dp.setLimiterAllChannelsTo(limiter)
            processors[sessionId] = dp
            _attachedSessionCount.value = processors.size
            _lastError.value = null
        } catch (e: Exception) {
            if (sessionId == 0) {
                _lastError.value = "Sesión global 0 restringida por la ROM; usando sesiones por app."
            } else {
                _lastError.value = "DynamicsProcessing sesión $sessionId: ${e.message}"
            }
        }
    }

    private fun detachSession(sessionId: Int) = synchronized(processorsLock) {
        processors.remove(sessionId)?.let { dp ->
            try {
                dp.enabled = false
                dp.release()
            } catch (_: Exception) {
            }
        }
        _attachedSessionCount.value = processors.size
    }

    fun refreshFromStore(updateNotification: Boolean) {
        val snap = store.loadSnapshot()
        val bands: List<FilterBand> = snap.bands
        val enabled = snap.dspEnabled && snap.btGlobalEqEnabled
        if (!enabled) {
            shutdownServiceCompletely()
            return
        }
        if (snap.btSessionZeroFallback) {
            attachSession(0)
        } else {
            detachSession(0)
        }
        val gains = BiquadMath.combinedResponseDb(bands, eqCenterFreqs)
        val maxBoost = gains.maxOrNull()?.let { max(0.0, it) } ?: 0.0
        val preampDb = (-maxBoost).coerceIn(-18.0, 0.0).toFloat()

        val eq = DynamicsProcessing.Eq(true, enabled, EQ_BAND_COUNT)
        for (i in 0 until EQ_BAND_COUNT) {
            val g = if (enabled) gains[i].coerceIn(-18.0, 18.0).toFloat() else 0f
            eq.setBand(i, DynamicsProcessing.EqBand(enabled, eqCutoffFreqs[i], g))
        }

        val failedSessions = mutableListOf<Int>()
        synchronized(processorsLock) {
            for ((sid, dp) in processors) {
                try {
                    dp.setInputGainAllChannelsTo(if (enabled) preampDb else 0f)
                    dp.setPostEqAllChannelsTo(eq)
                    dp.enabled = enabled
                } catch (e: Exception) {
                    if (sid != 0) failedSessions += sid
                    _lastError.value = "Error aplicando EQ en sesión $sid: ${e.message}"
                }
            }
            failedSessions.forEach { sid ->
                processors.remove(sid)?.let { dp ->
                    try {
                        dp.enabled = false
                        dp.release()
                    } catch (_: Exception) {
                    }
                }
            }
            _attachedSessionCount.value = processors.size
        }

        if (updateNotification) {
            val nm = getSystemService(Context.NOTIFICATION_SERVICE) as? NotificationManager
            nm?.notify(NOTIF_ID, buildNotification(snap.deviceName, snap.activePresetName, enabled))
        }
    }

    private fun ensureNotificationChannel() {
        val nm = getSystemService(Context.NOTIFICATION_SERVICE) as? NotificationManager ?: return
        val channel = NotificationChannel(
            CHANNEL_ID,
            "AutoQ Activo",
            NotificationManager.IMPORTANCE_LOW,
        ).apply {
            description = "Ecualizador paramétrico AutoQ en tiempo real para audífonos Bluetooth, USB-C y 3.5mm."
            setShowBadge(false)
        }
        nm.createNotificationChannel(channel)
    }

    private fun buildNotification(deviceName: String, presetName: String, eqOn: Boolean): Notification {
        val openIntent = Intent(this, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP
        }
        val pi = PendingIntent.getActivity(
            this,
            0,
            openIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        val stopIntent = Intent(this, GlobalEqService::class.java).setAction(ACTION_POWER_OFF)
        val stopPi = PendingIntent.getService(
            this,
            1,
            stopIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        val statusText = if (eqOn) "$presetName · EQ activo" else "EQ apagado"
        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setSmallIcon(android.R.drawable.ic_lock_silent_mode_off)
            .setContentTitle(deviceName.ifBlank { "AutoQ" })
            .setContentText(statusText)
            .setOngoing(true)
            .setSilent(true)
            .setContentIntent(pi)
            .addAction(android.R.drawable.ic_menu_close_clear_cancel, "Apagar EQ", stopPi)
            .build()
    }
}
