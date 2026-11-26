package com.hector.moondropdsp.data

import android.content.Context
import com.hector.moondropdsp.dsp.BiquadMath
import com.hector.moondropdsp.dsp.FilterBand
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

@Serializable
data class EqPreset(
    val name: String,
    val bands: List<FilterBand>,
    val builtIn: Boolean = false,
)

@Serializable
data class CurvePoint(
    val freqHz: Double,
    val db: Double,
)

@Serializable
data class PersistedAppState(
    val deviceName: String = "Sin dispositivo",
    val shortWidgetName: String = "AUTOQ",
    val transportBadge: String = "LISTO",
    val connected: Boolean = false,
    val dspEnabled: Boolean = true,
    val livePreview: Boolean = true,
    val overlayFrMode: Boolean = true,
    val activePresetName: String = "Plano (0 dB)",
    val bands: List<FilterBand> = BiquadMath.flatBands(BiquadMath.DEFAULT_VIRTUAL_BANDS),
    val stockFrCurve: List<CurvePoint> = emptyList(),
    val targetFrCurve: List<CurvePoint> = emptyList(),
    val targetLabel: String = "",
    val btGlobalEqEnabled: Boolean = true,
    val btSessionZeroFallback: Boolean = true,
    val btSavedBandsByKey: Map<String, List<FilterBand>> = emptyMap(),
    val customPresets: List<EqPreset> = emptyList(),
    val themeId: String = "pure_hyper",
    val accentId: String = "auto",
)

class AppPreferencesStore(context: Context) {

    private val prefs = context.applicationContext.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
    private val json = Json {
        ignoreUnknownKeys = true
        encodeDefaults = true
    }

    companion object {
        private const val PREFS_NAME = "moondrop_dsp_store"
        private const val KEY_STATE = "app_state_v1"

        val BUILT_IN_PRESETS: List<EqPreset> = listOf(
            EqPreset("Plano (0 dB)", BiquadMath.flatBands(BiquadMath.DEFAULT_VIRTUAL_BANDS), builtIn = true),
            EqPreset("Harman Warm", BiquadMath.HARMAN_WARM_BANDS, builtIn = true),
            EqPreset("Vocal / Ref", BiquadMath.VOCAL_REF_BANDS, builtIn = true),
            EqPreset("Bass Boost", BiquadMath.BASS_BOOST_BANDS, builtIn = true),
        )
    }

    fun loadSnapshot(): PersistedAppState {
        val raw = prefs.getString(KEY_STATE, null) ?: return PersistedAppState()
        return try {
            json.decodeFromString<PersistedAppState>(raw)
        } catch (_: Exception) {
            PersistedAppState()
        }
    }

    fun saveSnapshot(state: PersistedAppState) {
        prefs.edit().putString(KEY_STATE, json.encodeToString(state)).apply()
    }

    fun updateSnapshot(transform: (PersistedAppState) -> PersistedAppState): PersistedAppState {
        val updated = transform(loadSnapshot())
        saveSnapshot(updated)
        return updated
    }

    fun allPresets(state: PersistedAppState = loadSnapshot()): List<EqPreset> {
        val customNames = state.customPresets.map { it.name }.toSet()
        return BUILT_IN_PRESETS.filter { it.name !in customNames } + state.customPresets
    }
}
