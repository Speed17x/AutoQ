package com.hector.moondropdsp.ui

import android.app.Application
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.hardware.usb.UsbManager
import android.media.AudioDeviceCallback
import android.media.AudioDeviceInfo
import android.media.AudioManager
import android.os.Handler
import android.os.Looper
import androidx.core.content.ContextCompat
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.hector.moondropdsp.data.AppPreferencesStore
import com.hector.moondropdsp.data.AutoEqEntry
import com.hector.moondropdsp.data.AutoEqParser
import com.hector.moondropdsp.data.AutoEqRepository
import com.hector.moondropdsp.data.AutoEqResult
import com.hector.moondropdsp.data.CuratedTarget
import com.hector.moondropdsp.data.CurvePoint
import com.hector.moondropdsp.data.EqPreset
import com.hector.moondropdsp.dsp.BiquadMath
import com.hector.moondropdsp.dsp.FilterBand
import com.hector.moondropdsp.hardware.BluetoothMoondropTransport
import com.hector.moondropdsp.hardware.DeviceKind
import com.hector.moondropdsp.hardware.GlobalEqService
import com.hector.moondropdsp.hardware.MoondropDevice
import com.hector.moondropdsp.hardware.UsbDiagnosticEntry
import com.hector.moondropdsp.hardware.UsbMoondropTransport
import com.hector.moondropdsp.widget.NothingEqWidgetProvider
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

enum class AutoEqMode {
    TARGET_CURVE,
    MATCH_HEADPHONE,
}

data class MoondropUiState(
    val devices: List<MoondropDevice> = emptyList(),
    val activeDevice: MoondropDevice? = null,
    val bands: List<FilterBand> = BiquadMath.flatBands(BiquadMath.DEFAULT_VIRTUAL_BANDS),
    val selectedBandIdx: Int = 0,
    val dspEnabled: Boolean = true,
    val livePreview: Boolean = true,
    val overlayFrMode: Boolean = true,
    val activePresetName: String = "Plano (0 dB)",
    val presets: List<EqPreset> = AppPreferencesStore.BUILT_IN_PRESETS,
    val stockFrCurve: List<CurvePoint> = emptyList(),
    val targetFrCurve: List<CurvePoint> = emptyList(),
    val targetLabel: String = "",
    val statusMessage: String = "Listo · 10 bandas virtuales (o 5/8B en USB HID)",
    val statusError: Boolean = false,
    val busy: Boolean = false,
    // Multi-source AutoEQ + Squiglink state
    val autoEqLoadingIndex: Boolean = false,
    val autoEqTotalCount: Int = 0,
    val autoEqActiveSources: Int = 0,
    val sourceSearchQuery: String = "",
    val sourceSearchResults: List<AutoEqEntry> = emptyList(),
    val selectedSourceEntry: AutoEqEntry? = null,
    val autoEqMode: AutoEqMode = AutoEqMode.TARGET_CURVE,
    val selectedTargetIdx: Int = 1, // Harman In-Ear 2019
    val searchQuery: String = "",
    val searchResults: List<AutoEqEntry> = emptyList(),
    val selectedMatchEntry: AutoEqEntry? = null,
    val maxFreqFitHz: Double = 10000.0,
    val autoEqComputing: Boolean = false,
    val lastAutoEqResult: AutoEqResult? = null,
    // System EQ & Diagnostics
    val btGlobalEqEnabled: Boolean = true,
    val btSessionZeroFallback: Boolean = true,
    val btServiceRunning: Boolean = false,
    val btAttachedSessions: Int = 0,
    val btServiceError: String? = null,
    val hasBluetoothPermission: Boolean = false,
    val usbDiagnostics: List<UsbDiagnosticEntry> = emptyList(),
    val audioOutputDiagnostics: List<String> = emptyList(),
)

class MoondropViewModel(application: Application) : AndroidViewModel(application) {

    private val ctx: Context = application.applicationContext
    private val store = AppPreferencesStore(ctx)
    private val usbTransport = UsbMoondropTransport(ctx)
    private val btTransport = BluetoothMoondropTransport(ctx)
    private val autoEqRepo = AutoEqRepository(ctx)

    private var allAutoEqEntries: List<AutoEqEntry> = emptyList()
    private var livePushJob: Job? = null
    private var widgetSyncJob: Job? = null

    private val _uiState = MutableStateFlow(buildInitialState())
    val uiState: StateFlow<MoondropUiState> = _uiState.asStateFlow()

    private val audioManager = ctx.getSystemService(Context.AUDIO_SERVICE) as? AudioManager

    private val audioCallback = object : AudioDeviceCallback() {
        override fun onAudioDevicesAdded(addedDevices: Array<out AudioDeviceInfo>?) {
            refreshDevices(readFromHardware = false)
        }

        override fun onAudioDevicesRemoved(removedDevices: Array<out AudioDeviceInfo>?) {
            refreshDevices(readFromHardware = false)
        }
    }

    private val usbReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context?, intent: Intent?) {
            when (intent?.action) {
                UsbManager.ACTION_USB_DEVICE_ATTACHED,
                UsbManager.ACTION_USB_DEVICE_DETACHED,
                -> refreshDevices(readFromHardware = true)

                UsbMoondropTransport.ACTION_USB_PERMISSION -> {
                    val granted = intent.getBooleanExtra(UsbManager.EXTRA_PERMISSION_GRANTED, false)
                    if (granted) {
                        setStatus("Permiso USB concedido · Leyendo DSP...", isError = false)
                        refreshDevices(readFromHardware = true)
                    } else {
                        setStatus("Permiso USB denegado por el usuario.", isError = true)
                        refreshDevices(readFromHardware = false)
                    }
                }
            }
        }
    }

    init {
        viewModelScope.launch {
            GlobalEqService.running.collect { r ->
                _uiState.update { it.copy(btServiceRunning = r) }
            }
        }
        viewModelScope.launch {
            GlobalEqService.attachedSessionCount.collect { cnt ->
                _uiState.update { it.copy(btAttachedSessions = cnt) }
            }
        }
        viewModelScope.launch {
            GlobalEqService.lastError.collect { err ->
                _uiState.update { it.copy(btServiceError = err) }
            }
        }

        audioManager?.registerAudioDeviceCallback(audioCallback, Handler(Looper.getMainLooper()))

        val filter = IntentFilter().apply {
            addAction(UsbManager.ACTION_USB_DEVICE_ATTACHED)
            addAction(UsbManager.ACTION_USB_DEVICE_DETACHED)
            addAction(UsbMoondropTransport.ACTION_USB_PERMISSION)
        }
        ContextCompat.registerReceiver(ctx, usbReceiver, filter, ContextCompat.RECEIVER_NOT_EXPORTED)

        refreshDevices(readFromHardware = true)
    }

    override fun onCleared() {
        audioManager?.unregisterAudioDeviceCallback(audioCallback)
        try {
            ctx.unregisterReceiver(usbReceiver)
        } catch (_: Exception) {
        }
        super.onCleared()
    }

    private fun buildInitialState(): MoondropUiState {
        val snap = store.loadSnapshot()
        val initialBands = if (snap.bands.size >= BiquadMath.DEFAULT_VIRTUAL_BANDS) {
            snap.bands
        } else {
            fitBandCount(snap.bands.ifEmpty { BiquadMath.flatBands(BiquadMath.DEFAULT_VIRTUAL_BANDS) }, BiquadMath.DEFAULT_VIRTUAL_BANDS)
        }
        return MoondropUiState(
            bands = initialBands,
            dspEnabled = snap.dspEnabled,
            livePreview = snap.livePreview,
            overlayFrMode = snap.overlayFrMode,
            activePresetName = snap.activePresetName,
            presets = store.allPresets(snap),
            stockFrCurve = snap.stockFrCurve,
            targetFrCurve = snap.targetFrCurve,
            targetLabel = snap.targetLabel,
            btGlobalEqEnabled = snap.btGlobalEqEnabled,
            btSessionZeroFallback = snap.btSessionZeroFallback,
            hasBluetoothPermission = btTransport.hasBluetoothConnectPermission(),
        )
    }

    fun onPermissionsChanged() {
        _uiState.update { it.copy(hasBluetoothPermission = btTransport.hasBluetoothConnectPermission()) }
        refreshDevices(readFromHardware = false)
    }

    fun refreshDevices(readFromHardware: Boolean = true, preferredId: String? = null) {
        viewModelScope.launch {
            val usbDevs = withContext(Dispatchers.IO) { usbTransport.scanDevices() }
            val extDevs = withContext(Dispatchers.IO) { btTransport.scanConnectedExternalAudio(usbDevs) }
            val allDevs = usbDevs + extDevs
            val usbDiag = withContext(Dispatchers.IO) { usbTransport.inspectAllUsbDevices() }
            val audioDiag = withContext(Dispatchers.IO) { btTransport.listAllConnectedAudioOutputs() }

            val curId = preferredId ?: _uiState.value.activeDevice?.id
            val chosen = allDevs.firstOrNull { it.id == curId } ?: allDevs.firstOrNull()
            val targetBandCount = chosen?.maxBands ?: BiquadMath.DEFAULT_VIRTUAL_BANDS

            _uiState.update { st ->
                val adjustedBands = fitBandCount(st.bands, targetBandCount)
                st.copy(
                    devices = allDevs,
                    activeDevice = chosen,
                    bands = adjustedBands,
                    selectedBandIdx = st.selectedBandIdx.coerceIn(0, adjustedBands.lastIndex.coerceAtLeast(0)),
                    hasBluetoothPermission = btTransport.hasBluetoothConnectPermission(),
                    usbDiagnostics = usbDiag,
                    audioOutputDiagnostics = audioDiag,
                )
            }

            if (chosen == null) {
                persistAndSyncWidget(
                    status = "Sin audífonos externos · EQ global listo si activas el servicio",
                    isError = false,
                )
                return@launch
            }

            if (chosen.kind == DeviceKind.USB_HID) {
                if (!chosen.hasUsbPermission) {
                    persistAndSyncWidget(
                        status = "${chosen.displayName} detectado (${chosen.vidPid}) · Pulsa Autorizar USB",
                        isError = false,
                    )
                } else if (readFromHardware) {
                    readFromUsbDevice(chosen)
                } else {
                    persistAndSyncWidget("${chosen.displayName} conectado (${chosen.transportBadge})", false)
                }
            } else {
                // Any Bluetooth, generic USB-C DAC, or 3.5mm wired headset
                val snap = store.loadSnapshot()
                val key = chosen.btMac.ifBlank { chosen.id }
                val savedBands = snap.btSavedBandsByKey[key]
                if (savedBands != null && savedBands.isNotEmpty()) {
                    _uiState.update {
                        it.copy(
                            bands = fitBandCount(savedBands, chosen.maxBands),
                            selectedBandIdx = it.selectedBandIdx.coerceIn(0, chosen.maxBands - 1),
                        )
                    }
                }
                persistAndSyncWidget("${chosen.displayName} conectado (${chosen.transportBadge})", false)
                if (_uiState.value.btGlobalEqEnabled && _uiState.value.dspEnabled) {
                    GlobalEqService.startOrUpdate(ctx)
                }
            }

            if (allAutoEqEntries.isNotEmpty() && chosen.autoeqHint.isNotBlank()) {
                val matched = AutoEqParser.findBestEntryAnyBrand(allAutoEqEntries, chosen.autoeqHint)
                if (matched != null) {
                    _uiState.update { it.copy(selectedSourceEntry = matched) }
                    loadStockFrForSelectedSource()
                }
            }
        }
    }

    fun selectDevice(device: MoondropDevice) {
        refreshDevices(readFromHardware = true, preferredId = device.id)
    }

    fun requestUsbPermission() {
        val dev = _uiState.value.activeDevice ?: return
        if (dev.kind != DeviceKind.USB_HID) return
        val already = usbTransport.requestPermission(dev)
        if (already) {
            refreshDevices(readFromHardware = true)
        } else {
            setStatus("Acepta el diálogo de permiso USB del sistema...", isError = false)
        }
    }

    private suspend fun readFromUsbDevice(device: MoondropDevice) {
        _uiState.update { it.copy(busy = true) }
        try {
            val (enabled, readBands) = withContext(Dispatchers.IO) { usbTransport.readEq(device) }
            _uiState.update {
                it.copy(
                    bands = fitBandCount(readBands, device.maxBands),
                    selectedBandIdx = it.selectedBandIdx.coerceIn(0, device.maxBands - 1),
                    dspEnabled = enabled,
                    busy = false,
                )
            }
            persistAndSyncWidget(
                "Leído de ${device.displayName} (${device.maxBands} bandas · EQ ${if (enabled) "ON" else "OFF"})",
                false,
            )
        } catch (e: Exception) {
            _uiState.update { it.copy(busy = false) }
            setStatus(e.message ?: "Error leyendo DSP USB", isError = true)
        }
    }

    fun selectBand(index: Int) {
        _uiState.update { st ->
            st.copy(selectedBandIdx = index.coerceIn(0, st.bands.lastIndex.coerceAtLeast(0)))
        }
    }

    fun updateBand(index: Int, transform: (FilterBand) -> FilterBand) {
        val dev = _uiState.value.activeDevice
        val maxGain = dev?.maxGainDb ?: 12.0
        val allowShelves = dev?.supportsShelves ?: true
        _uiState.update { st ->
            if (index !in st.bands.indices) return@update st
            val next = st.bands.toMutableList()
            next[index] = transform(next[index]).clamped(maxGainDb = maxGain, allowShelves = allowShelves)
            st.copy(bands = next, activePresetName = "Personalizado")
        }
        scheduleThrottledPersistAndWidget()
        scheduleLivePushIfEnabled()
    }

    fun updateBandFromDrag(index: Int, newFreqHz: Double, newGainDb: Double) {
        updateBand(index) { b -> b.copy(frequency = newFreqHz, gain = newGainDb) }
    }

    fun onBandDragFinished() {
        if (_uiState.value.livePreview) {
            applyCurrentBands(commit = false)
        }
    }

    fun toggleLivePreview(enabled: Boolean) {
        _uiState.update { it.copy(livePreview = enabled) }
        persistAndSyncWidget(if (enabled) "Live preview activado" else "Live preview desactivado (pulsa Aplicar)", false)
    }

    fun toggleOverlayFrMode() {
        _uiState.update { it.copy(overlayFrMode = !it.overlayFrMode) }
        persistAndSyncWidget(null, false)
        if (_uiState.value.overlayFrMode && _uiState.value.stockFrCurve.isEmpty()) {
            ensureAutoEqIndexLoaded(silent = true)
        }
    }

    fun toggleDspEnabled() {
        val next = !_uiState.value.dspEnabled
        _uiState.update { it.copy(dspEnabled = next) }
        persistAndSyncWidget(if (next) "EQ activado" else "EQ en bypass (OFF)", false)
        applyCurrentBands(commit = false)
    }

    fun resetFlat() {
        val count = _uiState.value.activeDevice?.maxBands ?: BiquadMath.DEFAULT_VIRTUAL_BANDS
        _uiState.update {
            it.copy(
                bands = BiquadMath.flatBands(count),
                activePresetName = "Plano (0 dB)",
                targetFrCurve = emptyList(),
                targetLabel = "",
            )
        }
        persistAndSyncWidget("EQ reiniciado a plano (0 dB)", false)
        if (_uiState.value.livePreview) applyCurrentBands(commit = false)
    }

    private fun scheduleThrottledPersistAndWidget() {
        widgetSyncJob?.cancel()
        widgetSyncJob = viewModelScope.launch {
            delay(180)
            persistAndSyncWidget(null, false)
        }
    }

    private fun scheduleLivePushIfEnabled() {
        if (!_uiState.value.livePreview) return
        livePushJob?.cancel()
        livePushJob = viewModelScope.launch {
            delay(160)
            applyCurrentBands(commit = false, quiet = true)
        }
    }

    fun applyCurrentBands(commit: Boolean, quiet: Boolean = false) {
        val st = _uiState.value
        val dev = st.activeDevice

        viewModelScope.launch {
            if (!quiet) _uiState.update { it.copy(busy = true) }
            try {
                if (dev != null && dev.kind == DeviceKind.USB_HID) {
                    if (!dev.hasUsbPermission) {
                        if (!quiet) {
                            setStatus("Autoriza el permiso USB primero para escribir en ${dev.displayName}.", true)
                        }
                        return@launch
                    }
                    withContext(Dispatchers.IO) {
                        usbTransport.writeEq(dev, st.bands, commit = commit, enableEq = st.dspEnabled)
                    }
                    val msg = if (commit) {
                        "Guardado en flash de ${dev.displayName}" +
                            if (dev.disconnectsOnCommit) " (el DAC se reconectará en unos segundos)" else ""
                    } else {
                        "Aplicado en vivo en ${dev.displayName}"
                    }
                    persistAndSyncWidget(if (quiet) null else msg, false)
                    if (commit && dev.disconnectsOnCommit) {
                        delay(2200)
                        refreshDevices(readFromHardware = false)
                    }
                } else {
                    // System DynamicsProcessing EQ for ANY Bluetooth, generic USB-C, 3.5mm, or speaker output
                    if (dev != null) {
                        val key = dev.btMac.ifBlank { dev.id }
                        store.updateSnapshot { snap ->
                            snap.copy(
                                btSavedBandsByKey = snap.btSavedBandsByKey + (key to st.bands),
                                bands = st.bands,
                                dspEnabled = st.dspEnabled,
                            )
                        }
                    } else {
                        store.updateSnapshot { snap ->
                            snap.copy(bands = st.bands, dspEnabled = st.dspEnabled)
                        }
                    }
                    if (st.btGlobalEqEnabled) {
                        GlobalEqService.startOrUpdate(ctx, liveOnlyIfRunning = quiet)
                    }
                    val label = dev?.displayName ?: "audio del sistema"
                    persistAndSyncWidget(
                        if (quiet) null else "EQ (${st.bands.size}B) aplicado vía DynamicsProcessing para $label",
                        false,
                    )
                }
            } catch (e: Exception) {
                setStatus(e.message ?: "Error aplicando EQ", isError = true)
            } finally {
                if (!quiet) _uiState.update { it.copy(busy = false) }
            }
        }
    }

    // ---- Universal AutoEQ Database (8,850+ models across all brands) ----

    fun ensureAutoEqIndexLoaded(forceRefresh: Boolean = false, silent: Boolean = false) {
        if (!forceRefresh && allAutoEqEntries.isNotEmpty()) return
        viewModelScope.launch {
            _uiState.update { it.copy(autoEqLoadingIndex = true) }
            try {
                val summary = autoEqRepo.loadCatalog(forceRefresh = forceRefresh)
                val entries = summary.entries
                allAutoEqEntries = entries
                val hint = _uiState.value.activeDevice?.autoeqHint.orEmpty()
                val autoMatched = AutoEqParser.findBestEntryAnyBrand(entries, hint)
                    ?: _uiState.value.selectedSourceEntry
                    ?: entries.firstOrNull()

                val srcQuery = _uiState.value.sourceSearchQuery
                val tgtQuery = _uiState.value.searchQuery
                _uiState.update {
                    it.copy(
                        autoEqLoadingIndex = false,
                        autoEqTotalCount = entries.size,
                        autoEqActiveSources = summary.activeSourceCount,
                        selectedSourceEntry = autoMatched,
                        sourceSearchResults = filterEntries(entries, srcQuery),
                        searchResults = filterEntries(entries, tgtQuery),
                    )
                }
                if (!silent) {
                    setStatus("Catálogo listo: ${entries.size} curvas (${summary.activeSourceCount} fuentes AutoEQ + Squiglink)", false)
                }
                loadStockFrForSelectedSource()
            } catch (e: Exception) {
                _uiState.update { it.copy(autoEqLoadingIndex = false) }
                if (!silent) setStatus("Error cargando bases de curvas: ${e.message}", true)
            }
        }
    }

    fun updateSourceSearch(query: String) {
        _uiState.update {
            it.copy(
                sourceSearchQuery = query,
                sourceSearchResults = filterEntries(allAutoEqEntries, query),
            )
        }
    }

    fun selectSourceEntry(entry: AutoEqEntry) {
        _uiState.update { it.copy(selectedSourceEntry = entry) }
        loadStockFrForSelectedSource()
    }

    private fun loadStockFrForSelectedSource() {
        val src = _uiState.value.selectedSourceEntry ?: return
        viewModelScope.launch {
            try {
                val fr = autoEqRepo.reconstructStockFr(src)
                _uiState.update { it.copy(stockFrCurve = fr) }
                persistAndSyncWidget(null, false)
            } catch (_: Exception) {
            }
        }
    }

    fun setAutoEqMode(mode: AutoEqMode) {
        _uiState.update { it.copy(autoEqMode = mode) }
    }

    fun selectTargetCurve(index: Int) {
        _uiState.update { it.copy(selectedTargetIdx = index.coerceIn(0, AutoEqParser.CURATED_TARGETS.lastIndex)) }
    }

    fun updateAutoEqSearch(query: String) {
        _uiState.update {
            it.copy(
                searchQuery = query,
                searchResults = filterEntries(allAutoEqEntries, query),
            )
        }
    }

    fun selectMatchHeadphone(entry: AutoEqEntry) {
        _uiState.update { it.copy(selectedMatchEntry = entry) }
    }

    fun setMaxFreqFit(hz: Double) {
        _uiState.update { it.copy(maxFreqFitHz = hz) }
    }

    fun computeAutoEq(applyImmediately: Boolean = true) {
        val st = _uiState.value
        val src = st.selectedSourceEntry
        if (src == null) {
            setStatus("Primero busca y selecciona tu audífono actual en la base AutoEQ.", true)
            return
        }
        val dev = st.activeDevice
        val maxBands = dev?.maxBands ?: BiquadMath.DEFAULT_VIRTUAL_BANDS
        val maxGain = dev?.maxGainDb ?: 12.0
        val shelves = dev?.supportsShelves ?: true

        viewModelScope.launch {
            _uiState.update { it.copy(autoEqComputing = true) }
            try {
                val result: AutoEqResult = when (st.autoEqMode) {
                    AutoEqMode.TARGET_CURVE -> {
                        val target: CuratedTarget = AutoEqParser.CURATED_TARGETS.getOrElse(st.selectedTargetIdx) {
                            AutoEqParser.CURATED_TARGETS.first()
                        }
                        autoEqRepo.computeTargetCurvePeq(
                            source = src,
                            target = target,
                            maxBands = maxBands,
                            maxFreqFit = st.maxFreqFitHz,
                            maxGainDb = maxGain,
                            supportsShelves = shelves,
                        )
                    }

                    AutoEqMode.MATCH_HEADPHONE -> {
                        val tgtPhone = st.selectedMatchEntry
                            ?: throw IllegalStateException("Selecciona un audífono objetivo en el buscador.")
                        autoEqRepo.computeMatchHeadphonePeq(
                            source = src,
                            targetPhone = tgtPhone,
                            maxBands = maxBands,
                            maxFreqFit = st.maxFreqFitHz,
                            maxGainDb = maxGain,
                            supportsShelves = shelves,
                        )
                    }
                }

                _uiState.update {
                    it.copy(
                        autoEqComputing = false,
                        lastAutoEqResult = result,
                        bands = if (applyImmediately) result.bands else it.bands,
                        activePresetName = if (applyImmediately) "AutoQ · ${result.targetLabel.take(18)}" else it.activePresetName,
                        dspEnabled = if (applyImmediately) true else it.dspEnabled,
                        stockFrCurve = result.stockFrCurve,
                        targetFrCurve = result.targetFrCurve,
                        targetLabel = result.targetLabel,
                        selectedBandIdx = 0,
                    )
                }

                val rmseTxt = String.format(java.util.Locale.US, "%.2f dB", result.rmseDb)
                persistAndSyncWidget("AutoQ listo: ${result.targetLabel} (RMSE $rmseTxt)", false)
                if (applyImmediately && _uiState.value.livePreview) {
                    applyCurrentBands(commit = false)
                }
            } catch (e: Exception) {
                _uiState.update { it.copy(autoEqComputing = false) }
                setStatus(e.message ?: "Error calculando AutoEQ", true)
            }
        }
    }

    // ---- Presets & EqualizerAPO ----

    fun applyPreset(preset: EqPreset) {
        val count = _uiState.value.activeDevice?.maxBands ?: BiquadMath.DEFAULT_VIRTUAL_BANDS
        _uiState.update {
            it.copy(
                bands = fitBandCount(preset.bands, count),
                activePresetName = preset.name,
                dspEnabled = true,
                targetFrCurve = emptyList(),
                targetLabel = "",
                selectedBandIdx = 0,
            )
        }
        persistAndSyncWidget("Preset cargado: ${preset.name}", false)
        applyCurrentBands(commit = false)
    }

    fun saveCustomPreset(rawName: String) {
        val name = rawName.trim()
        if (name.isBlank()) return
        val newPreset = EqPreset(name = name, bands = _uiState.value.bands, builtIn = false)
        val updated = store.updateSnapshot { snap ->
            val filtered = snap.customPresets.filterNot { it.name.equals(name, ignoreCase = true) }
            snap.copy(
                customPresets = filtered + newPreset,
                activePresetName = name,
            )
        }
        _uiState.update {
            it.copy(
                presets = store.allPresets(updated),
                activePresetName = name,
            )
        }
        NothingEqWidgetProvider.updateAllWidgets(ctx)
        setStatus("Preset guardado: $name (sincronizado con el widget)", false)
    }

    fun deleteCustomPreset(name: String) {
        val updated = store.updateSnapshot { snap ->
            snap.copy(customPresets = snap.customPresets.filterNot { it.name == name })
        }
        _uiState.update { it.copy(presets = store.allPresets(updated)) }
        NothingEqWidgetProvider.updateAllWidgets(ctx)
        setStatus("Preset eliminado: $name", false)
    }

    fun exportParametricEqText(): String = AutoEqParser.exportParametricEq(_uiState.value.bands)

    fun importParametricEqText(text: String) {
        val parsed = AutoEqParser.parseParametricEq(text)
        if (parsed.isEmpty()) {
            setStatus("No se encontraron líneas 'Filter N: ON PK/LSC/HSC ...' válidas.", true)
            return
        }
        val maxBands = _uiState.value.activeDevice?.maxBands ?: BiquadMath.DEFAULT_VIRTUAL_BANDS
        _uiState.update {
            it.copy(
                bands = fitBandCount(parsed, maxBands),
                activePresetName = "Importado APO",
                dspEnabled = true,
                selectedBandIdx = 0,
            )
        }
        persistAndSyncWidget("Importadas ${minOf(parsed.size, maxBands)} bandas desde EqualizerAPO", false)
        if (_uiState.value.livePreview) applyCurrentBands(commit = false)
    }

    // ---- System EQ Service & Optional Moondrop SPP ----

    fun setBtGlobalEqEnabled(enabled: Boolean) {
        _uiState.update { it.copy(btGlobalEqEnabled = enabled) }
        persistAndSyncWidget(null, false)
        if (enabled) {
            GlobalEqService.startOrUpdate(ctx)
            setStatus("Servicio EQ global (DynamicsProcessing 32B) activado", false)
        } else {
            GlobalEqService.stop(ctx)
            setStatus("Servicio EQ global detenido", false)
        }
    }

    fun setBtSessionZeroFallback(enabled: Boolean) {
        _uiState.update { it.copy(btSessionZeroFallback = enabled) }
        persistAndSyncWidget(null, false)
        if (_uiState.value.btGlobalEqEnabled && _uiState.value.btServiceRunning) {
            GlobalEqService.startOrUpdate(ctx)
        }
    }

    fun btSppReadEq() {
        val dev = _uiState.value.activeDevice
        if (dev == null || dev.kind != DeviceKind.BT_AUDIO || dev.btMac.isBlank()) {
            setStatus("Conecta un TWS compatible por Bluetooth con permiso BLUETOOTH_CONNECT.", true)
            return
        }
        viewModelScope.launch {
            _uiState.update { it.copy(busy = true) }
            try {
                val readBands = withContext(Dispatchers.IO) { btTransport.sppReadEq(dev.btMac) }
                _uiState.update {
                    it.copy(
                        bands = fitBandCount(readBands, dev.maxBands),
                        activePresetName = "${dev.shortWidgetName} SPP",
                        busy = false,
                    )
                }
                persistAndSyncWidget("PEQ leído por Bluetooth SPP (${readBands.size} bandas)", false)
            } catch (e: Exception) {
                _uiState.update { it.copy(busy = false) }
                setStatus(e.message ?: "Error en Bluetooth SPP", true)
            }
        }
    }

    fun btSppWriteEq() {
        val dev = _uiState.value.activeDevice
        if (dev == null || dev.kind != DeviceKind.BT_AUDIO || dev.btMac.isBlank()) {
            setStatus("Conecta un TWS compatible por Bluetooth con permiso BLUETOOTH_CONNECT.", true)
            return
        }
        viewModelScope.launch {
            _uiState.update { it.copy(busy = true) }
            try {
                withContext(Dispatchers.IO) { btTransport.sppWriteEq(dev.btMac, _uiState.value.bands) }
                _uiState.update { it.copy(busy = false) }
                setStatus("PEQ de 5 bandas enviado al DSP interno vía SPP", false)
            } catch (e: Exception) {
                _uiState.update { it.copy(busy = false) }
                setStatus(e.message ?: "Error escribiendo por Bluetooth SPP", true)
            }
        }
    }

    fun btSppSelectFirmwarePreset(index: Int, label: String) {
        val dev = _uiState.value.activeDevice
        if (dev == null || dev.kind != DeviceKind.BT_AUDIO || dev.btMac.isBlank()) {
            setStatus("Conecta un TWS compatible por Bluetooth primero.", true)
            return
        }
        viewModelScope.launch {
            _uiState.update { it.copy(busy = true) }
            try {
                withContext(Dispatchers.IO) { btTransport.sppSetPreset(dev.btMac, index) }
                _uiState.update { it.copy(busy = false) }
                setStatus("Preset interno TWS cambiado a $label", false)
            } catch (e: Exception) {
                _uiState.update { it.copy(busy = false) }
                setStatus(e.message ?: "Error cambiando preset SPP", true)
            }
        }
    }

    private fun setStatus(message: String, isError: Boolean) {
        _uiState.update { it.copy(statusMessage = message, statusError = isError) }
    }

    private fun persistAndSyncWidget(status: String?, isError: Boolean) {
        val st = _uiState.value
        val dev = st.activeDevice
        store.updateSnapshot { snap ->
            snap.copy(
                deviceName = dev?.displayName ?: "AutoQ",
                shortWidgetName = dev?.shortWidgetName ?: "AUTOQ",
                transportBadge = dev?.transportBadge ?: "LISTO",
                connected = dev != null,
                dspEnabled = st.dspEnabled,
                livePreview = st.livePreview,
                overlayFrMode = st.overlayFrMode,
                activePresetName = st.activePresetName,
                bands = st.bands,
                stockFrCurve = st.stockFrCurve,
                targetFrCurve = st.targetFrCurve,
                targetLabel = st.targetLabel,
                btGlobalEqEnabled = st.btGlobalEqEnabled,
                btSessionZeroFallback = st.btSessionZeroFallback,
            )
        }
        if (status != null) {
            _uiState.update { it.copy(statusMessage = status, statusError = isError) }
        }
        NothingEqWidgetProvider.updateAllWidgets(ctx)
    }

    private fun fitBandCount(bands: List<FilterBand>, targetCount: Int): List<FilterBand> {
        val n = targetCount.coerceAtLeast(1)
        if (bands.size == n) return bands
        val base = BiquadMath.flatBands(n)
        return List(n) { i -> bands.getOrElse(i) { base[i] } }
    }

    private fun filterEntries(entries: List<AutoEqEntry>, query: String): List<AutoEqEntry> {
        if (entries.isEmpty()) return emptyList()
        val normQuery = AutoEqParser.normalizeSearchText(query)
        val tokens = normQuery.split(' ').filter { it.isNotBlank() }
        if (tokens.isEmpty()) return entries.take(80)
        val out = ArrayList<AutoEqEntry>(80)
        for (e in entries) {
            val hay = AutoEqParser.normalizeSearchText("${e.name} ${e.sourceRig}")
            if (tokens.all { it in hay }) {
                out += e
                if (out.size >= 80) break
            }
        }
        return out
    }
}
