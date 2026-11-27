import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.plasma.plasmoid
import org.kde.plasma.core as PlasmaCore
import org.kde.plasma.components as PlasmaComponents3
import org.kde.kirigami as Kirigami
import org.kde.plasma.plasma5support as Plasma5Support

PlasmoidItem {
    id: root

    Plasmoid.backgroundHints: PlasmaCore.Types.NoBackground
    switchWidth: Kirigami.Units.gridUnit * 14
    switchHeight: Kirigami.Units.gridUnit * 14

    readonly property string triviumCliCmd: "PATH=\"$HOME/.local/bin:$PATH\" trivium-cli"
    readonly property string monoFont: "Consolas, Inconsolata, monospace"
    property color cyberBg: Qt.rgba(0.055, 0.067, 0.086, 0.92)
    property color cyberPanel: Qt.rgba(0.090, 0.106, 0.133, 0.90)
    property color borderCol: "#28303D"
    property color textPrimary: "#F2F5FA"
    property color textMuted: "#949FB2"
    property color cyanGlow: "#949FB2"
    property color amberTarget: "#D8E2F2"
    property color coralAccent: "#3482FF"
    property bool themeIsDark: true
    property bool useDotGrid: false

    property var activeDevice: null
    property var allDevices: []
    property var bands: [
        { type: "peaking", frequency: 31, gain: 0.0, q: 1.0 },
        { type: "peaking", frequency: 63, gain: 0.0, q: 1.0 },
        { type: "peaking", frequency: 125, gain: 0.0, q: 1.0 },
        { type: "peaking", frequency: 250, gain: 0.0, q: 1.0 },
        { type: "peaking", frequency: 500, gain: 0.0, q: 1.0 },
        { type: "peaking", frequency: 1000, gain: 0.0, q: 1.0 },
        { type: "peaking", frequency: 2000, gain: 0.0, q: 1.0 },
        { type: "peaking", frequency: 4000, gain: 0.0, q: 1.0 },
        { type: "peaking", frequency: 8000, gain: 0.0, q: 1.0 },
        { type: "peaking", frequency: 16000, gain: 0.0, q: 1.0 }
    ]
    property int selectedBand: 0
    property bool dspEnabled: true
    property bool livePreview: true
    property bool showAutoEq: false
    property bool pickingSource: false
    property bool showApoBox: false
    property bool overlayFrMode: true
    property int activeTab: 0
    property string statusMessage: "READY"
    property bool statusError: false
    property bool busy: false
    property bool pipewireActive: false
    property bool dirtyBands: false
    property bool forceReadBands: true
    property string lastRmseText: ""
    property string targetLabel: ""

    property int totalDbCount: 0
    property int activeSources: 0
    property int totalSources: 8
    property var selectedSourceEntry: null
    property var sourceSearchResults: []
    property var targetList: []
    property var presetsMap: ({})
    property var presetNames: []
    property var searchResults: []
    property var targetCurvePoints: []    // [[freq, delta_db], ...]
    property var stockFrPoints: []        // [[freq, fr_db], ...]
    property var targetFrPoints: []       // [[freq, fr_db], ...]
    property string lastDeviceKey: ""
    property string lastPollFingerprint: ""
    property var pendingTags: ({})

    function toB64(str) {
        return Qt.btoa(unescape(encodeURIComponent(str)));
    }

    function runBackend(tag, args) {
        var cmd = triviumCliCmd + " " + args;
        pendingTags[cmd] = tag;
        execSource.connectSource(cmd);
    }

    function runFastPoll() {
        var cmd = triviumCliCmd + " poll";
        pendingTags[cmd] = "poll";
        execSource.connectSource(cmd);
    }

    function refreshStatus(forceDb, readBands) {
        if (readBands) forceReadBands = true;
        if (forceDb) {
            busy = true;
            statusMessage = "SYNCING 8 FR DATABASES...";
            statusError = false;
        }
        var devArg = activeDevice ? (" --device-id " + activeDevice.id) : "";
        var refArg = forceDb ? " --refresh-db" : "";
        runBackend("status", "status" + devArg + refArg);
    }

    function selectBand(idx) {
        if (idx === selectedBand) return;
        if (typeof freqSpin !== "undefined" && freqSpin.activeFocus) {
            var fParsed = parseInt(freqSpin.contentItem.text);
            if (!isNaN(fParsed)) {
                updateBandField(selectedBand, "frequency", Math.max(20, Math.min(20000, fParsed)));
            }
        }
        if (typeof qSpin !== "undefined" && qSpin.activeFocus) {
            var qParsed = parseFloat(qSpin.contentItem.text.replace(/[^0-9.]/g, ""));
            if (!isNaN(qParsed)) {
                updateBandField(selectedBand, "q", Math.max(0.1, Math.min(10.0, qParsed)));
            }
        }
        selectedBand = Math.max(0, Math.min(idx, bands.length - 1));
    }

    function fitBandsToDevice(rawBands) {
        var maxB = activeDevice ? activeDevice.max_bands : 10;
        var defFreqs = maxB <= 5
            ? [60, 250, 1000, 3500, 8000]
            : [31, 63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];
        var out = [];
        for (var i = 0; i < Math.min(rawBands.length, maxB); i++) {
            out.push(Object.assign({}, rawBands[i]));
        }
        while (out.length < maxB) {
            out.push({ type: "peaking", frequency: defFreqs[out.length % defFreqs.length], gain: 0.0, q: 1.0 });
        }
        return out;
    }

    function pickSourceEntry(entry) {
        selectedSourceEntry = entry;
        pickingSource = false;
        if (entry && entry.rel_path) {
            runBackend("sourcefr", "source-fr --src-b64 " + toB64(entry.rel_path));
        }
    }

    function applyBands(commitFlag) {
        if (!activeDevice) {
            statusMessage = "NO DEVICE // CONNECT USB, BT OR AUDIO SINK";
            statusError = true;
            return;
        }
        var b64 = toB64(JSON.stringify(bands));
        var cmd = "apply --device-id " + activeDevice.id + " --bands-b64 " + b64
            + (commitFlag ? " --commit" : "")
            + (dspEnabled ? "" : " --bypass");
        if (commitFlag) busy = true;
        runBackend(commitFlag ? "commit" : "apply", cmd);
    }

    function toggleDspEnabled() {
        dspEnabled = !dspEnabled;
        if (activeDevice) {
            applyBands(false);
        }
    }

    function setFlat() {
        targetCurvePoints = [];
        targetFrPoints = [];
        targetLabel = "";
        lastRmseText = "";
        dspEnabled = true;
        var devArg = activeDevice ? (" --device-id " + activeDevice.id) : "";
        runBackend("flat", "flat" + devArg);
    }

    function runSearch(q, ieOnly) {
        var cmd = "search --q-b64 " + toB64(q) + (ieOnly ? " --in-ear-only" : "") + " --limit 40";
        runBackend("search", cmd);
    }

    function runSourceSearch(q) {
        var cmd = "search --q-b64 " + toB64(q) + " --limit 35";
        runBackend("srcsearch", cmd);
    }

    function computeMatch(tgtRel, maxFreq) {
        if (!selectedSourceEntry || !selectedSourceEntry.rel_path) {
            statusMessage = "ELIGE PRIMERO TU AUDÍFONO (SRC)";
            statusError = true;
            pickingSource = true;
            runSourceSearch("");
            return;
        }
        var srcRel = selectedSourceEntry.rel_path;
        var maxB = activeDevice ? activeDevice.max_bands : 10;
        var maxG = activeDevice ? activeDevice.max_gain_db : 12.0;
        var noSh = (activeDevice && !activeDevice.supports_shelves) ? " --no-shelves" : "";
        busy = true;
        statusMessage = "FITTING " + maxB + " BANDS...";
        statusError = false;
        runBackend(
            "autoeq",
            "autoeq-match --src-b64 " + toB64(srcRel) +
            " --tgt-b64 " + toB64(tgtRel) +
            " --max-freq " + maxFreq +
            " --bands " + maxB +
            " --max-gain " + maxG + noSh
        );
    }

    function computeTarget(targetCsv, maxFreq) {
        if (!selectedSourceEntry || !selectedSourceEntry.rel_path) {
            statusMessage = "ELIGE PRIMERO TU AUDÍFONO (SRC)";
            statusError = true;
            pickingSource = true;
            runSourceSearch("");
            return;
        }
        var srcRel = selectedSourceEntry.rel_path;
        var maxB = activeDevice ? activeDevice.max_bands : 10;
        var maxG = activeDevice ? activeDevice.max_gain_db : 12.0;
        var noSh = (activeDevice && !activeDevice.supports_shelves) ? " --no-shelves" : "";
        busy = true;
        statusMessage = "SOLVING TARGET (" + maxB + "B)...";
        statusError = false;
        runBackend(
            "autoeq",
            "autoeq-target --src-b64 " + toB64(srcRel) +
            " --target-b64 " + toB64(targetCsv) +
            " --max-freq " + maxFreq +
            " --bands " + maxB +
            " --max-gain " + maxG + noSh
        );
    }

    function importApoText(txt) {
        var maxB = activeDevice ? activeDevice.max_bands : 10;
        runBackend("importapo", "import-apo --text-b64 " + toB64(txt) + " --bands " + maxB);
    }

    function exportApoText() {
        runBackend("exportapo", "export-apo --bands-b64 " + toB64(JSON.stringify(bands)));
    }

    function updateBandField(idx, field, val) {
        var copy = bands.slice();
        var b = Object.assign({}, copy[idx]);
        b[field] = val;
        copy[idx] = b;
        bands = copy;
        dirtyBands = true;
        if (livePreview && activeDevice && dspEnabled) {
            previewTimer.restart();
        }
    }

    function interpCurveAt(pts, freq) {
        if (!pts || !pts.length) return 0.0;
        if (freq <= pts[0][0]) return pts[0][1];
        if (freq >= pts[pts.length - 1][0]) return pts[pts.length - 1][1];
        var lf = Math.log(freq);
        for (var i = 1; i < pts.length; i++) {
            if (freq <= pts[i][0]) {
                var lf0 = Math.log(pts[i - 1][0]);
                var lf1 = Math.log(pts[i][0]);
                var t = (lf - lf0) / Math.max(1e-9, lf1 - lf0);
                return pts[i - 1][1] + t * (pts[i][1] - pts[i - 1][1]);
            }
        }
        return 0.0;
    }

    function biquadDbAt(band, freq) {
        var g = band.gain;
        if (Math.abs(g) < 0.01) return 0.0;
        var fs = 48000.0;
        var f0 = Math.max(10.0, Math.min(22000.0, band.frequency));
        var q = Math.max(0.1, Math.min(10.0, band.q));
        var A = Math.pow(10.0, g / 40.0);
        var w0 = 2.0 * Math.PI * f0 / fs;
        var cw = Math.cos(w0);
        var sw = Math.sin(w0);
        var alpha = sw / (2.0 * q);
        var b0, b1, b2, a0, a1, a2;
        if (band.type === "low_shelf") {
            var s = 2.0 * Math.sqrt(A) * alpha;
            b0 = A * ((A + 1) - (A - 1) * cw + s);
            b1 = 2 * A * ((A - 1) - (A + 1) * cw);
            b2 = A * ((A + 1) - (A - 1) * cw - s);
            a0 = (A + 1) + (A - 1) * cw + s;
            a1 = -2 * ((A - 1) + (A + 1) * cw);
            a2 = (A + 1) + (A - 1) * cw - s;
        } else if (band.type === "high_shelf") {
            var s2 = 2.0 * Math.sqrt(A) * alpha;
            b0 = A * ((A + 1) + (A - 1) * cw + s2);
            b1 = -2 * A * ((A - 1) + (A + 1) * cw);
            b2 = A * ((A + 1) + (A - 1) * cw - s2);
            a0 = (A + 1) - (A - 1) * cw + s2;
            a1 = 2 * ((A - 1) - (A + 1) * cw);
            a2 = (A + 1) - (A - 1) * cw - s2;
        } else {
            b0 = 1 + alpha * A;
            b1 = -2 * cw;
            b2 = 1 - alpha * A;
            a0 = 1 + alpha / A;
            a1 = -2 * cw;
            a2 = 1 - alpha / A;
        }
        var b0n = b0 / a0, b1n = b1 / a0, b2n = b2 / a0;
        var a1n = a1 / a0, a2n = a2 / a0;
        var w = 2.0 * Math.PI * freq / fs;
        var phi = Math.pow(Math.sin(w / 2.0), 2);
        var num = Math.pow(b0n + b1n + b2n, 2) - 4 * (b0n * b1n + 4 * b0n * b2n + b1n * b2n) * phi + 16 * b0n * b2n * phi * phi;
        var den = Math.pow(1 + a1n + a2n, 2) - 4 * (a1n + 4 * a2n + a1n * a2n) * phi + 16 * a2n * phi * phi;
        return 10.0 * Math.log(Math.max(num, 1e-18) / Math.max(den, 1e-18)) / Math.LN10;
    }

    function totalEqAt(freq) {
        if (!dspEnabled) return 0.0;
        var sumDb = 0.0;
        for (var i = 0; i < bands.length; i++) sumDb += biquadDbAt(bands[i], freq);
        return sumDb;
    }

    Plasma5Support.DataSource {
        id: execSource
        engine: "executable"
        connectedSources: []
        onNewData: (sourceName, data) => {
            disconnectSource(sourceName);
            var tag = pendingTags[sourceName] || "";
            delete pendingTags[sourceName];
            var raw = (data["stdout"] || "").trim();
            if (tag === "poll") {
                if (raw && raw !== lastPollFingerprint) {
                    var hadFp = (lastPollFingerprint !== "");
                    lastPollFingerprint = raw;
                    if (hadFp) refreshStatus(false, true);
                }
                return;
            }
            if (!raw) {
                busy = false;
                if (data["stderr"]) {
                    statusMessage = data["stderr"].trim().split("\n")[0];
                    statusError = true;
                }
                return;
            }
            try {
                var res = JSON.parse(raw);
                if (!res.ok) {
                    busy = false;
                    statusMessage = res.error || "ERROR";
                    statusError = true;
                    return;
                }
                if (tag === "status") {
                    busy = false;
                    allDevices = res.devices || [];
                    activeDevice = res.device || null;
                    pipewireActive = !!res.pipewire_active;
                    if (res.dsp_enabled !== undefined) dspEnabled = !!res.dsp_enabled;
                    totalDbCount = res.total_db_count || 0;
                    activeSources = res.active_sources || 0;
                    totalSources = res.total_sources || 8;

                    var devKey = activeDevice ? activeDevice.id : "none";
                    var changedDev = (devKey !== lastDeviceKey);
                    lastDeviceKey = devKey;

                    var shouldLoadBands = forceReadBands || changedDev;
                    forceReadBands = false;

                    if (shouldLoadBands && res.bands && res.bands.length) {
                        bands = res.bands;
                        dirtyBands = false;
                        if (selectedBand >= bands.length) selectedBand = 0;
                    }
                    if (res.auto_source_entry && (changedDev || !selectedSourceEntry)) {
                        selectedSourceEntry = res.auto_source_entry;
                    }
                    if (res.stock_fr_curve && res.stock_fr_curve.length && (changedDev || !stockFrPoints.length)) {
                        stockFrPoints = res.stock_fr_curve;
                    }
                    if (res.targets) targetList = res.targets;
                    if (res.presets) {
                        presetsMap = res.presets;
                        presetNames = Object.keys(res.presets);
                    }
                    if (res.ui_settings) {
                        var ui = res.ui_settings;
                        root.themeIsDark = (ui.is_dark !== undefined) ? !!ui.is_dark : true;
                        root.useDotGrid = !!ui.use_dot_grid;
                        if (ui.bg) {
                            var bgc = Qt.color(ui.bg);
                            root.cyberBg = Qt.rgba(bgc.r, bgc.g, bgc.b, 0.94);
                        }
                        if (ui.panel) {
                            var pnc = Qt.color(ui.panel);
                            root.cyberPanel = Qt.rgba(pnc.r, pnc.g, pnc.b, 0.90);
                        }
                        if (ui.border) root.borderCol = Qt.color(ui.border);
                        if (ui.accent_hex) root.coralAccent = Qt.color(ui.accent_hex);
                        if (ui.text_primary) {
                            root.textPrimary = Qt.color(ui.text_primary);
                            root.amberTarget = Qt.color(ui.text_primary);
                        }
                        if (ui.text_muted) {
                            root.textMuted = Qt.color(ui.text_muted);
                            root.cyanGlow = Qt.color(ui.text_muted);
                        }
                        eqCanvas.requestPaint();
                    }
                    if (res.read_error) {
                        statusMessage = res.read_error;
                        statusError = true;
                    } else if (!dirtyBands) {
                        if (activeDevice) {
                            statusMessage = "LINK OK // " + activeDevice.transport_label + " · " + totalDbCount + " CURVAS (" + activeSources + "/" + totalSources + ")";
                            statusError = false;
                        } else {
                            statusMessage = "STANDBY // SIN DISPOSITIVO DE AUDIO";
                            statusError = false;
                        }
                    }
                } else if (tag === "sourcefr") {
                    if (res.stock_fr_curve) stockFrPoints = res.stock_fr_curve;
                    if (res.entry) statusMessage = "SRC: " + res.entry.name;
                    statusError = false;
                } else if (tag === "apply" || tag === "commit" || tag === "flat") {
                    busy = false;
                    if (res.bands && res.bands.length) bands = res.bands;
                    if (res.dsp_enabled !== undefined) dspEnabled = !!res.dsp_enabled;
                    if (tag === "commit" || tag === "flat" || (activeDevice && activeDevice.kind !== "usb_hid")) {
                        dirtyBands = false;
                    }
                    if (res.pipewire_active !== undefined) pipewireActive = !!res.pipewire_active;
                    statusMessage = (res.message || "SYNCED").toUpperCase();
                    statusError = false;
                } else if (tag === "search") {
                    searchResults = res.results || [];
                } else if (tag === "srcsearch") {
                    sourceSearchResults = res.results || [];
                } else if (tag === "autoeq") {
                    busy = false;
                    if (res.bands) {
                        bands = res.bands;
                        if (selectedBand >= bands.length) selectedBand = 0;
                        dirtyBands = true;
                    }
                    dspEnabled = true;
                    if (res.target_curve) targetCurvePoints = res.target_curve;
                    if (res.stock_fr_curve) stockFrPoints = res.stock_fr_curve;
                    if (res.target_fr_curve) targetFrPoints = res.target_fr_curve;
                    if (res.rmse !== undefined) lastRmseText = "RMSE " + res.rmse.toFixed(2) + " dB";
                    targetLabel = res.target_label || "";
                    statusMessage = res.warning ? (res.message + " ⚠") : (res.message || "AUTOEQ COMPUTED");
                    statusError = false;
                    if (livePreview && activeDevice) {
                        applyBands(false);
                    }
                } else if (tag === "preset") {
                    if (res.presets) {
                        presetsMap = res.presets;
                        presetNames = Object.keys(res.presets);
                    }
                    if (res.ui_settings) {
                        var ui = res.ui_settings;
                        root.themeIsDark = (ui.is_dark !== undefined) ? !!ui.is_dark : true;
                        root.useDotGrid = !!ui.use_dot_grid;
                        if (ui.bg) {
                            var bgc = Qt.color(ui.bg);
                            root.cyberBg = Qt.rgba(bgc.r, bgc.g, bgc.b, 0.94);
                        }
                        if (ui.panel) {
                            var pnc = Qt.color(ui.panel);
                            root.cyberPanel = Qt.rgba(pnc.r, pnc.g, pnc.b, 0.90);
                        }
                        if (ui.border) root.borderCol = Qt.color(ui.border);
                        if (ui.accent_hex) root.coralAccent = Qt.color(ui.accent_hex);
                        if (ui.text_primary) {
                            root.textPrimary = Qt.color(ui.text_primary);
                            root.amberTarget = Qt.color(ui.text_primary);
                        }
                        if (ui.text_muted) {
                            root.textMuted = Qt.color(ui.text_muted);
                            root.cyanGlow = Qt.color(ui.text_muted);
                        }
                        eqCanvas.requestPaint();
                    }
                    statusMessage = res.message || "PRESET OK";
                    statusError = false;
                } else if (tag === "importapo") {
                    if (res.bands && res.bands.length) {
                        bands = res.bands;
                        if (selectedBand >= bands.length) selectedBand = 0;
                        dspEnabled = true;
                        dirtyBands = true;
                        showApoBox = false;
                        if (livePreview && activeDevice) applyBands(false);
                    }
                    statusMessage = res.message || "APO IMPORTADO";
                    statusError = false;
                } else if (tag === "exportapo") {
                    apoTextArea.text = res.text || "";
                    showApoBox = true;
                    apoTextArea.selectAll();
                    apoTextArea.copy();
                    statusMessage = "PARAMETRICEQ.TXT COPIADO AL PORTAPAPELES";
                    statusError = false;
                } else if (tag === "btspp") {
                    busy = false;
                    if (res.bands && res.bands.length) bands = res.bands;
                    statusMessage = res.message || "BT SPP OK";
                    statusError = false;
                }
            } catch (e) {
                busy = false;
                statusMessage = "PARSE ERR: " + e;
                statusError = true;
            }
        }
    }

    Timer {
        id: previewTimer
        interval: 180
        repeat: false
        onTriggered: root.applyBands(false)
    }

    Timer {
        interval: 4000
        running: true
        repeat: true
        onTriggered: {
            if (!root.busy) root.runFastPoll();
        }
    }

    Component.onCompleted: {
        runFastPoll();
        refreshStatus(false, true);
    }

    compactRepresentation: MouseArea {
        id: compactMouse
        hoverEnabled: true
        onClicked: root.expanded = !root.expanded
        Kirigami.Icon {
            anchors.centerIn: parent
            width: Math.min(parent.width, parent.height) * 0.85
            height: width
            source: "audio-headphones"
            color: (root.activeDevice && root.dspEnabled) ? root.coralAccent : Kirigami.Theme.textColor
        }
        Rectangle {
            visible: !!root.activeDevice
            width: 7
            height: 7
            radius: 3.5
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            anchors.margins: 3
            color: root.dspEnabled ? root.coralAccent : "#64748B"
        }
        QQC2.ToolTip.text: root.activeDevice
            ? ("TRIVIUM // " + root.activeDevice.display_name + " (" + root.activeDevice.transport_label + ")")
            : "TRIVIUM // OFFLINE"
        QQC2.ToolTip.visible: compactMouse.containsMouse
    }

    fullRepresentation: Item {
        id: fullView
        readonly property int extraBandH: root.bands.length > 5 ? 38 : 0
        implicitWidth: Kirigami.Units.gridUnit * 26.5
        implicitHeight: (root.showAutoEq ? Kirigami.Units.gridUnit * 37.5 : Kirigami.Units.gridUnit * 22.0) + extraBandH

        Rectangle {
            id: hudShell
            anchors.fill: parent
            radius: 10
            color: root.cyberBg
            border.width: 1
            border.color: root.borderCol

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 11
                spacing: 7

                // ── HUD Telemetry Header ────────────────────────────────────
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 5

                    Rectangle {
                        width: 8
                        height: 8
                        radius: 4
                        color: (root.activeDevice && root.dspEnabled) ? root.coralAccent : "#475569"
                    }

                    ColumnLayout {
                        spacing: 0
                        Layout.fillWidth: true
                        PlasmaComponents3.Label {
                            text: root.activeDevice ? root.activeDevice.display_name : "Trivium"
                            font.family: root.monoFont
                            font.bold: true
                            font.pixelSize: 12
                            color: root.textPrimary
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                        PlasmaComponents3.Label {
                            text: root.activeDevice
                                ? (root.activeDevice.transport_label + " · " + root.activeDevice.max_bands + " bandas" + (root.dspEnabled ? "" : " · BYPASS"))
                                : "Sin dispositivo conectado"
                            font.family: root.monoFont
                            font.pixelSize: 10
                            color: root.activeDevice ? (root.dspEnabled ? root.coralAccent : root.textMuted) : root.textMuted
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                    }

                    PlasmaComponents3.ComboBox {
                        visible: root.allDevices.length > 1
                        model: root.allDevices
                        textRole: "display_name"
                        implicitWidth: 112
                        onActivated: idx => {
                            root.activeDevice = root.allDevices[idx];
                            root.refreshStatus(false, true);
                        }
                    }

                    // DSP ON/OFF bypass switch
                    PlasmaComponents3.ToolButton {
                        text: root.dspEnabled ? "ON" : "OFF"
                        font.family: root.monoFont
                        font.bold: true
                        font.pixelSize: 10
                        checked: root.dspEnabled
                        QQC2.ToolTip.text: "Activar o desactivar (bypass) el ecualizador"
                        QQC2.ToolTip.visible: hovered
                        onClicked: root.toggleDspEnabled()
                    }

                    // Overlay Mode Switch: FR vs ΔEQ
                    PlasmaComponents3.ToolButton {
                        text: root.overlayFrMode ? "FR" : "ΔEQ"
                        font.family: root.monoFont
                        font.bold: true
                        font.pixelSize: 10
                        checked: root.overlayFrMode
                        QQC2.ToolTip.text: "Alternar entre curvas acústicas superpuestas (FR) o solo ganancia EQ (ΔEQ)"
                        QQC2.ToolTip.visible: hovered
                        onClicked: root.overlayFrMode = !root.overlayFrMode
                    }

                    PlasmaComponents3.ToolButton {
                        text: "0dB"
                        font.family: root.monoFont
                        font.pixelSize: 10
                        QQC2.ToolTip.text: "Resetear EQ a plano (0 dB)"
                        QQC2.ToolTip.visible: hovered
                        onClicked: root.setFlat()
                    }

                    PlasmaComponents3.ToolButton {
                        icon.name: "view-refresh"
                        QQC2.ToolTip.text: "Re-escanear dispositivos y leer EQ"
                        QQC2.ToolTip.visible: hovered
                        onClicked: root.refreshStatus(false, true)
                    }

                    PlasmaComponents3.ToolButton {
                        text: root.showAutoEq ? "DB ▴" : "DB ▾"
                        font.family: root.monoFont
                        font.bold: true
                        font.pixelSize: 10
                        checked: root.showAutoEq
                        onClicked: {
                            root.showAutoEq = !root.showAutoEq;
                            if (root.showAutoEq && root.searchResults.length === 0) {
                                root.runSearch("", false);
                            }
                        }
                    }
                }

                // ── Overlaid Acoustic Frequency Response Canvas ─────────────
                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: Kirigami.Units.gridUnit * 8.8
                    radius: 7
                    color: root.cyberPanel
                    border.color: root.borderCol
                    border.width: 1

                    Canvas {
                        id: eqCanvas
                        anchors.fill: parent
                        anchors.margins: 3

                        readonly property var bandsSnap: root.bands
                        readonly property var tgtSnap: root.targetCurvePoints
                        readonly property var stockFrSnap: root.stockFrPoints
                        readonly property var tgtFrSnap: root.targetFrPoints
                        readonly property bool frModeSnap: root.overlayFrMode
                        readonly property bool dspSnap: root.dspEnabled
                        readonly property int selSnap: root.selectedBand
                        readonly property color accentSnap: root.coralAccent
                        readonly property bool dotGridSnap: root.useDotGrid
                        readonly property bool darkSnap: root.themeIsDark

                        onBandsSnapChanged: requestPaint()
                        onTgtSnapChanged: requestPaint()
                        onStockFrSnapChanged: requestPaint()
                        onTgtFrSnapChanged: requestPaint()
                        onFrModeSnapChanged: requestPaint()
                        onDspSnapChanged: requestPaint()
                        onSelSnapChanged: requestPaint()
                        onAccentSnapChanged: requestPaint()
                        onDotGridSnapChanged: requestPaint()
                        onDarkSnapChanged: requestPaint()
                        onWidthChanged: requestPaint()
                        onHeightChanged: requestPaint()

                        function dbMin() { return root.overlayFrMode ? -12.0 : -15.0; }
                        function dbMax() { return root.overlayFrMode ? 17.0 : 15.0; }

                        function fx(freq, padL, plotW) {
                            var lo = Math.log(20) / Math.LN10;
                            var hi = Math.log(20000) / Math.LN10;
                            var lf = Math.log(Math.max(20, Math.min(20000, freq))) / Math.LN10;
                            return padL + ((lf - lo) / (hi - lo)) * plotW;
                        }
                        function xf(x, padL, plotW) {
                            var lo = Math.log(20) / Math.LN10;
                            var hi = Math.log(20000) / Math.LN10;
                            var t = Math.max(0, Math.min(1, (x - padL) / plotW));
                            return Math.pow(10, lo + t * (hi - lo));
                        }
                        function gy(db, padT, plotH) {
                            var lo = dbMin(), hi = dbMax();
                            return padT + (1.0 - (db - lo) / (hi - lo)) * plotH;
                        }
                        function yg(y, padT, plotH) {
                            var lo = dbMin(), hi = dbMax();
                            return lo + (1.0 - (y - padT) / plotH) * (hi - lo);
                        }

                        onPaint: {
                            var ctx = getContext("2d");
                            ctx.clearRect(0, 0, width, height);
                            var padL = 26, padR = 8, padT = 16, padB = 16;
                            var plotW = Math.max(20, width - padL - padR);
                            var plotH = Math.max(20, height - padT - padB);

                            var coral = root.dspEnabled ? root.coralAccent : Qt.color("#64748B");
                            var cyan = Qt.color(root.cyanGlow);
                            var amber = Qt.color(root.amberTarget);

                            // Grid lines
                            var gridF = [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000];
                            var lblF = { 20: "20", 100: "100", 500: "500", 1000: "1k", 5000: "5k", 20000: "20k" };
                            ctx.font = '9px "' + root.monoFont + '", monospace';
                            ctx.textAlign = "center";
                            var gridAlpha = root.themeIsDark ? 0.08 : 0.14;
                            var gridRgb = root.themeIsDark ? "255,255,255" : "15,23,42";
                            if (root.useDotGrid) {
                                ctx.fillStyle = "rgba(" + gridRgb + ",0.16)";
                                for (var r = 0; r <= 6; r++) {
                                    var dy = padT + (r / 6) * plotH;
                                    for (var c = 0; c <= 18; c++) {
                                        var dx = padL + (c / 18) * plotW;
                                        ctx.beginPath();
                                        ctx.arc(dx, dy, 1.0, 0, Math.PI * 2);
                                        ctx.fill();
                                    }
                                }
                            }
                            for (var i = 0; i < gridF.length; i++) {
                                var x = fx(gridF[i], padL, plotW);
                                if (!root.useDotGrid) {
                                    ctx.strokeStyle = "rgba(" + gridRgb + "," + gridAlpha + ")";
                                    ctx.lineWidth = 1;
                                    ctx.beginPath();
                                    ctx.moveTo(x, padT);
                                    ctx.lineTo(x, padT + plotH);
                                    ctx.stroke();
                                }
                                if (lblF[gridF[i]]) {
                                    ctx.fillStyle = root.textMuted;
                                    ctx.fillText(lblF[gridF[i]], x, height - 3);
                                }
                            }

                            var gridDb = root.overlayFrMode ? [-10, -5, 0, 5, 10, 15] : [-12, -6, 0, 6, 12];
                            ctx.textAlign = "right";
                            for (var j = 0; j < gridDb.length; j++) {
                                var y = gy(gridDb[j], padT, plotH);
                                if (!root.useDotGrid || gridDb[j] === 0) {
                                    ctx.strokeStyle = "rgba(" + gridRgb + "," + (gridDb[j] === 0 ? 0.30 : gridAlpha) + ")";
                                    ctx.lineWidth = gridDb[j] === 0 ? 1.1 : 0.9;
                                    ctx.beginPath();
                                    ctx.moveTo(padL, y);
                                    ctx.lineTo(padL + plotW, y);
                                    ctx.stroke();
                                }
                                if (gridDb[j] === 0 || gridDb[j] === 10 || gridDb[j] === -10 || Math.abs(gridDb[j]) === 12) {
                                    ctx.fillStyle = root.textMuted;
                                    ctx.fillText((gridDb[j] > 0 ? "+" : "") + gridDb[j], padL - 4, y + 3);
                                }
                            }

                            var N = 84;
                            var hasStock = root.overlayFrMode && root.stockFrPoints && root.stockFrPoints.length > 1;

                            if (hasStock) {
                                ctx.save();
                                ctx.setLineDash([5, 3]);
                                ctx.strokeStyle = Qt.rgba(cyan.r, cyan.g, cyan.b, 0.58);
                                ctx.lineWidth = 1.5;
                                ctx.beginPath();
                                for (var s = 0; s < root.stockFrPoints.length; s++) {
                                    var sx = fx(root.stockFrPoints[s][0], padL, plotW);
                                    var sy = gy(root.stockFrPoints[s][1], padT, plotH);
                                    if (s === 0) ctx.moveTo(sx, sy);
                                    else ctx.lineTo(sx, sy);
                                }
                                ctx.stroke();
                                ctx.restore();
                            }

                            var overlayTarget = root.overlayFrMode ? root.targetFrPoints : root.targetCurvePoints;
                            if (overlayTarget && overlayTarget.length > 1) {
                                ctx.save();
                                ctx.setLineDash([2, 3]);
                                ctx.strokeStyle = Qt.rgba(amber.r, amber.g, amber.b, 0.88);
                                ctx.lineWidth = 1.8;
                                ctx.beginPath();
                                for (var t = 0; t < overlayTarget.length; t++) {
                                    var tx = fx(overlayTarget[t][0], padL, plotW);
                                    var ty = gy(overlayTarget[t][1], padT, plotH);
                                    if (t === 0) ctx.moveTo(tx, ty);
                                    else ctx.lineTo(tx, ty);
                                }
                                ctx.stroke();
                                ctx.restore();
                            }

                            var pts = [];
                            for (var k = 0; k < N; k++) {
                                var fr = Math.pow(10, (Math.log(20) / Math.LN10) + (k / (N - 1)) * (Math.log(20000 / 20) / Math.LN10));
                                var val = root.totalEqAt(fr);
                                if (hasStock) val += root.interpCurveAt(root.stockFrPoints, fr);
                                pts.push([fx(fr, padL, plotW), gy(val, padT, plotH)]);
                            }

                            var baseFillY = root.overlayFrMode ? (padT + plotH) : gy(0, padT, plotH);
                            ctx.beginPath();
                            ctx.moveTo(pts[0][0], baseFillY);
                            for (var m = 0; m < pts.length; m++) ctx.lineTo(pts[m][0], pts[m][1]);
                            ctx.lineTo(pts[pts.length - 1][0], baseFillY);
                            ctx.closePath();
                            ctx.fillStyle = Qt.rgba(coral.r, coral.g, coral.b, 0.14);
                            ctx.fill();

                            ctx.beginPath();
                            for (var g = 0; g < pts.length; g++) {
                                if (g === 0) ctx.moveTo(pts[g][0], pts[g][1]);
                                else ctx.lineTo(pts[g][0], pts[g][1]);
                            }
                            ctx.strokeStyle = Qt.rgba(coral.r, coral.g, coral.b, 0.35);
                            ctx.lineWidth = 5.0;
                            ctx.stroke();

                            ctx.strokeStyle = coral;
                            ctx.lineWidth = 2.1;
                            ctx.stroke();

                            for (var bi = 0; bi < root.bands.length; bi++) {
                                var band = root.bands[bi];
                                var bx = fx(band.frequency, padL, plotW);
                                var nodeVal = (root.dspEnabled ? band.gain : 0.0) + (hasStock ? root.interpCurveAt(root.stockFrPoints, band.frequency) : 0.0);
                                var by = gy(nodeVal, padT, plotH);
                                var sel = (bi === root.selectedBand);
                                ctx.beginPath();
                                ctx.arc(bx, by, sel ? 5.8 : 3.8, 0, Math.PI * 2);
                                ctx.fillStyle = sel ? coral : cyan;
                                ctx.fill();
                                if (sel) {
                                    ctx.strokeStyle = "#FFFFFF";
                                    ctx.lineWidth = 1.5;
                                    ctx.stroke();
                                }
                            }

                            ctx.textAlign = "left";
                            ctx.font = 'bold 9px "' + root.monoFont + '", monospace';
                            ctx.fillStyle = coral;
                            ctx.fillText(root.dspEnabled ? "━ LIVE" : "━ BYPASS", padL + 4, 11);
                            if (hasStock) {
                                ctx.fillStyle = cyan;
                                ctx.fillText("╌ STOCK", padL + 60, 11);
                            }
                            if (overlayTarget && overlayTarget.length > 1) {
                                ctx.fillStyle = amber;
                                ctx.fillText("┈ " + (root.targetLabel ? root.targetLabel.slice(0, 14).toUpperCase() : "TARGET"), padL + 114, 11);
                            }
                            if (root.lastRmseText) {
                                ctx.textAlign = "right";
                                ctx.fillStyle = cyan;
                                ctx.fillText(root.lastRmseText, padL + plotW - 2, 11);
                            }
                        }

                        MouseArea {
                            anchors.fill: parent
                            property int dragIdx: -1
                            onPressed: mouse => {
                                var padL = 26, padR = 8, padT = 16, padB = 16;
                                var plotW = Math.max(20, width - padL - padR);
                                var plotH = Math.max(20, height - padT - padB);
                                var hasStock = root.overlayFrMode && root.stockFrPoints && root.stockFrPoints.length > 1;
                                var best = -1, bestD2 = 16 * 16;
                                for (var i = 0; i < root.bands.length; i++) {
                                    var bx = eqCanvas.fx(root.bands[i].frequency, padL, plotW);
                                    var nodeVal = (root.dspEnabled ? root.bands[i].gain : 0.0) + (hasStock ? root.interpCurveAt(root.stockFrPoints, root.bands[i].frequency) : 0.0);
                                    var by = eqCanvas.gy(nodeVal, padT, plotH);
                                    var d2 = Math.pow(mouse.x - bx, 2) + Math.pow(mouse.y - by, 2);
                                    if (d2 < bestD2) { bestD2 = d2; best = i; }
                                }
                                if (best >= 0) {
                                    root.selectBand(best);
                                    dragIdx = best;
                                }
                            }
                            onPositionChanged: mouse => {
                                if (dragIdx < 0) return;
                                var padL = 26, padR = 8, padT = 16, padB = 16;
                                var plotW = Math.max(20, width - padL - padR);
                                var plotH = Math.max(20, height - padT - padB);
                                var newFreq = Math.round(eqCanvas.xf(mouse.x, padL, plotW));
                                var yVal = eqCanvas.yg(mouse.y, padT, plotH);
                                var hasStock = root.overlayFrMode && root.stockFrPoints && root.stockFrPoints.length > 1;
                                var gainVal = yVal - (hasStock ? root.interpCurveAt(root.stockFrPoints, newFreq) : 0.0);
                                var maxG = root.activeDevice ? root.activeDevice.max_gain_db : 12.0;
                                gainVal = Math.round(Math.max(-maxG, Math.min(maxG, gainVal)) * 10) / 10;

                                var copy = root.bands.slice();
                                var b = Object.assign({}, copy[dragIdx]);
                                b.frequency = newFreq;
                                b.gain = gainVal;
                                copy[dragIdx] = b;
                                root.bands = copy;
                                root.dirtyBands = true;
                                if (root.livePreview && root.activeDevice && root.dspEnabled) previewTimer.restart();
                            }
                            onReleased: dragIdx = -1
                        }
                    }
                }

                // ── Monospace Band Selector Grid (5B = 1 row, 8B/10B = 2 rows) ──
                GridLayout {
                    Layout.fillWidth: true
                    columns: 5
                    rowSpacing: 4
                    columnSpacing: 4
                    Repeater {
                        model: root.bands.length
                        delegate: Rectangle {
                            required property int index
                            Layout.fillWidth: true
                            Layout.preferredHeight: 34
                            radius: 5
                            color: root.selectedBand === index
                                ? Qt.rgba(root.coralAccent.r, root.coralAccent.g, root.coralAccent.b, 0.22)
                                : root.cyberPanel
                            border.width: 1
                            border.color: root.selectedBand === index
                                ? root.coralAccent
                                : root.borderCol

                            ColumnLayout {
                                anchors.centerIn: parent
                                spacing: 0
                                PlasmaComponents3.Label {
                                    Layout.alignment: Qt.AlignHCenter
                                    text: "B" + (index + 1) + " " + (root.bands[index].frequency >= 1000
                                        ? (root.bands[index].frequency / 1000).toFixed(1) + "k"
                                        : Math.round(root.bands[index].frequency))
                                    font.family: root.monoFont
                                    font.pixelSize: 9
                                    color: root.textMuted
                                }
                                PlasmaComponents3.Label {
                                    Layout.alignment: Qt.AlignHCenter
                                    text: (root.bands[index].gain >= 0 ? "+" : "") + root.bands[index].gain.toFixed(1)
                                    font.family: root.monoFont
                                    font.bold: true
                                    font.pixelSize: 11
                                    color: Math.abs(root.bands[index].gain) >= 0.1 ? root.coralAccent : root.textMuted
                                }
                            }
                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.PointingHandCursor
                                onClicked: root.selectBand(index)
                            }
                        }
                    }
                }

                // ── Selected Band Telemetry Editor ──────────────────────────
                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: bandEditorCol.implicitHeight + 12
                    radius: 6
                    color: root.cyberPanel
                    border.width: 1
                    border.color: Qt.rgba(0.39, 0.45, 0.55, 0.25)

                    ColumnLayout {
                        id: bandEditorCol
                        anchors.fill: parent
                        anchors.margins: 6
                        spacing: 4

                        readonly property var cur: root.bands[Math.min(root.selectedBand, root.bands.length - 1)]

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 5

                            PlasmaComponents3.ComboBox {
                                enabled: !root.activeDevice || root.activeDevice.supports_shelves
                                model: ["PK", "LS", "HS"]
                                readonly property var codes: ["peaking", "low_shelf", "high_shelf"]
                                currentIndex: Math.max(0, codes.indexOf(bandEditorCol.cur.type))
                                onActivated: idx => root.updateBandField(root.selectedBand, "type", codes[idx])
                                implicitWidth: 68
                            }

                            PlasmaComponents3.SpinBox {
                                id: freqSpin
                                from: 20
                                to: 20000
                                stepSize: 25
                                editable: true
                                value: Math.round(bandEditorCol.cur.frequency)
                                textFromValue: (v, loc) => v + " Hz"
                                valueFromText: (t, loc) => parseInt(t) || 1000
                                onValueModified: root.updateBandField(root.selectedBand, "frequency", value)
                                Layout.fillWidth: true
                            }

                            PlasmaComponents3.SpinBox {
                                id: qSpin
                                from: 10
                                to: 1000
                                stepSize: 10
                                editable: true
                                value: Math.round(bandEditorCol.cur.q * 100)
                                textFromValue: (v, loc) => "Q " + (v / 100).toFixed(2)
                                valueFromText: (t, loc) => Math.round((parseFloat(t.replace(/[^0-9.]/g, "")) || 1.0) * 100)
                                onValueModified: root.updateBandField(root.selectedBand, "q", value / 100.0)
                                implicitWidth: 112
                            }

                            PlasmaComponents3.ToolButton {
                                text: "0"
                                font.family: root.monoFont
                                onClicked: root.updateBandField(root.selectedBand, "gain", 0.0)
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8
                            PlasmaComponents3.Label {
                                text: "GAIN"
                                font.family: root.monoFont
                                font.pixelSize: 10
                                color: root.cyanGlow
                            }
                            PlasmaComponents3.Slider {
                                Layout.fillWidth: true
                                from: root.activeDevice ? -root.activeDevice.max_gain_db : -12.0
                                to: root.activeDevice ? root.activeDevice.max_gain_db : 12.0
                                stepSize: 0.1
                                value: bandEditorCol.cur.gain
                                onMoved: root.updateBandField(root.selectedBand, "gain", Math.round(value * 10) / 10)
                            }
                            PlasmaComponents3.Label {
                                Layout.preferredWidth: 56
                                horizontalAlignment: Text.AlignRight
                                font.family: root.monoFont
                                font.bold: true
                                color: root.coralAccent
                                text: (bandEditorCol.cur.gain >= 0 ? "+" : "") + bandEditorCol.cur.gain.toFixed(1) + "dB"
                            }
                        }
                    }
                }

                // ── Hardware / PipeWire Action Row ──────────────────────────
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 6

                    PlasmaComponents3.CheckBox {
                        text: "LIVE"
                        font.family: root.monoFont
                        font.pixelSize: 10
                        checked: root.livePreview
                        onToggled: root.livePreview = checked
                    }

                    Item { Layout.fillWidth: true }

                    PlasmaComponents3.ComboBox {
                        visible: !!root.activeDevice && !!root.activeDevice.supports_spp && !!root.activeDevice.mac
                        model: ["FW: Pop", "FW: Mon", "FW: Bass"]
                        implicitWidth: 96
                        onActivated: idx => {
                            root.runBackend("btspp", "bt-spp --mac " + root.activeDevice.mac + " --action preset --preset-idx " + idx);
                        }
                    }

                    PlasmaComponents3.Button {
                        text: ((root.activeDevice && root.activeDevice.kind === "usb_hid") ? "FLASH" : "APPLY PW") + (root.dirtyBands ? " *" : "")
                        font.family: root.monoFont
                        font.bold: true
                        highlighted: true
                        enabled: !root.busy && !!root.activeDevice
                        onClicked: root.applyBands(true)
                    }
                }

                // ── Trivium Universal Catalog & Presets Drawer ────────────────
                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    visible: root.showAutoEq
                    spacing: 6

                    // Universal Source Picker Bar + Sync + Max-Freq
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 4

                        PlasmaComponents3.Button {
                            Layout.fillWidth: true
                            text: root.selectedSourceEntry
                                ? ("SRC: " + root.selectedSourceEntry.name)
                                : "SRC: Elegir tu audífono..."
                            font.family: root.monoFont
                            font.pixelSize: 10
                            checked: root.pickingSource
                            onClicked: {
                                root.pickingSource = !root.pickingSource;
                                if (root.pickingSource) {
                                    root.runSourceSearch(srcSearchField.text);
                                }
                            }
                        }

                        PlasmaComponents3.ToolButton {
                            text: "↻"
                            font.family: root.monoFont
                            font.bold: true
                            font.pixelSize: 11
                            enabled: !root.busy
                            QQC2.ToolTip.text: "Sincronizar 8 fuentes (AutoEq + 7 Squiglink · " + root.totalDbCount + " curvas)"
                            QQC2.ToolTip.visible: hovered
                            onClicked: root.refreshStatus(true, false)
                        }

                        PlasmaComponents3.ComboBox {
                            id: maxFreqCombo
                            implicitWidth: 78
                            model: [
                                { text: "≤6k", val: 6000 },
                                { text: "≤8k", val: 8000 },
                                { text: "≤10k", val: 10000 },
                                { text: "≤14k", val: 14000 },
                                { text: "≤20k", val: 20000 }
                            ]
                            textRole: "text"
                            currentIndex: 2
                        }
                    }

                    // Inline Universal Source Search Drawer
                    Rectangle {
                        visible: root.pickingSource
                        Layout.fillWidth: true
                        Layout.preferredHeight: 138
                        radius: 6
                        color: root.cyberPanel
                        border.width: 1
                        border.color: root.cyanGlow

                        ColumnLayout {
                            anchors.fill: parent
                            anchors.margins: 6
                            spacing: 4

                            PlasmaComponents3.TextField {
                                id: srcSearchField
                                Layout.fillWidth: true
                                font.family: root.monoFont
                                font.pixelSize: 11
                                placeholderText: "Buscar tu audífono (Golden Ages 2, Zero 3, Chu 2, XM5)..."
                                onTextChanged: root.runSourceSearch(text)
                            }

                            QQC2.ScrollView {
                                Layout.fillWidth: true
                                Layout.fillHeight: true
                                clip: true
                                ListView {
                                    id: srcListView
                                    model: root.sourceSearchResults
                                    delegate: PlasmaComponents3.ItemDelegate {
                                        required property var modelData
                                        width: srcListView.width
                                        text: modelData.label
                                        font.family: root.monoFont
                                        font.pixelSize: 10
                                        onClicked: root.pickSourceEntry(modelData)
                                    }
                                }
                            }
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 4
                        PlasmaComponents3.Button {
                            Layout.fillWidth: true
                            text: "MATCH"
                            font.family: root.monoFont
                            font.pixelSize: 10
                            checkable: true
                            checked: root.activeTab === 0
                            onClicked: root.activeTab = 0
                        }
                        PlasmaComponents3.Button {
                            Layout.fillWidth: true
                            text: "TARGET"
                            font.family: root.monoFont
                            font.pixelSize: 10
                            checkable: true
                            checked: root.activeTab === 1
                            onClicked: root.activeTab = 1
                        }
                        PlasmaComponents3.Button {
                            Layout.fillWidth: true
                            text: "PRESETS"
                            font.family: root.monoFont
                            font.pixelSize: 10
                            checkable: true
                            checked: root.activeTab === 2
                            onClicked: root.activeTab = 2
                        }
                    }

                    // Tab 0: Match any headphone from all 14,500+ curves
                    ColumnLayout {
                        visible: root.activeTab === 0
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        spacing: 5

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 4
                            PlasmaComponents3.TextField {
                                id: searchField
                                Layout.fillWidth: true
                                font.family: root.monoFont
                                font.pixelSize: 11
                                placeholderText: "Imitar audífono (Zero 3, Hexa, Blessing 3, HD 600)..."
                                onTextChanged: root.runSearch(text, ieOnlyChk.checked)
                            }
                            PlasmaComponents3.CheckBox {
                                id: ieOnlyChk
                                text: "IEM"
                                font.family: root.monoFont
                                font.pixelSize: 10
                                checked: false
                                onToggled: root.runSearch(searchField.text, checked)
                            }
                        }

                        QQC2.ScrollView {
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            Layout.minimumHeight: Kirigami.Units.gridUnit * 4.5
                            clip: true

                            ListView {
                                id: phoneListView
                                model: root.searchResults
                                currentIndex: 0
                                delegate: PlasmaComponents3.ItemDelegate {
                                    required property var modelData
                                    required property int index
                                    width: phoneListView.width
                                    text: modelData.label
                                    font.family: root.monoFont
                                    font.pixelSize: 10
                                    highlighted: phoneListView.currentIndex === index
                                    onClicked: phoneListView.currentIndex = index
                                    onDoubleClicked: {
                                        phoneListView.currentIndex = index;
                                        root.computeMatch(modelData.rel_path, maxFreqCombo.model[maxFreqCombo.currentIndex].val);
                                    }
                                }
                            }
                        }

                        PlasmaComponents3.Button {
                            Layout.fillWidth: true
                            text: "CALCULAR AUTOEQ // IMITAR (" + (root.activeDevice ? root.activeDevice.max_bands : 10) + "B)"
                            font.family: root.monoFont
                            font.bold: true
                            highlighted: true
                            enabled: !root.busy && root.searchResults.length > 0 && phoneListView.currentIndex >= 0
                            onClicked: {
                                var item = root.searchResults[phoneListView.currentIndex];
                                root.computeMatch(item.rel_path, maxFreqCombo.model[maxFreqCombo.currentIndex].val);
                            }
                        }
                    }

                    // Tab 1: Target curves (8 curated targets)
                    ColumnLayout {
                        visible: root.activeTab === 1
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        spacing: 6
                        PlasmaComponents3.ComboBox {
                            id: targetCombo
                            Layout.fillWidth: true
                            implicitWidth: 200
                            model: root.targetList
                            textRole: "label"
                        }
                        PlasmaComponents3.Button {
                            Layout.fillWidth: true
                            text: "CALCULAR CURVA TARGET (" + (root.activeDevice ? root.activeDevice.max_bands : 10) + "B)"
                            font.family: root.monoFont
                            font.bold: true
                            highlighted: true
                            enabled: !root.busy && root.targetList.length > 0
                            onClicked: {
                                var t = root.targetList[targetCombo.currentIndex];
                                root.computeTarget(t.file, maxFreqCombo.model[maxFreqCombo.currentIndex].val);
                            }
                        }
                        Item { Layout.fillHeight: true }
                    }

                    // Tab 2: Presets + EqualizerAPO Import/Export + BT SPP
                    ColumnLayout {
                        visible: root.activeTab === 2
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        spacing: 5

                        RowLayout {
                            Layout.fillWidth: true
                            PlasmaComponents3.ComboBox {
                                id: presetCombo
                                Layout.fillWidth: true
                                implicitWidth: 160
                                model: root.presetNames
                            }
                            PlasmaComponents3.Button {
                                text: "LOAD"
                                font.family: root.monoFont
                                font.pixelSize: 10
                                enabled: presetCombo.currentIndex >= 0
                                onClicked: {
                                    var name = root.presetNames[presetCombo.currentIndex];
                                    if (root.presetsMap[name]) {
                                        root.targetCurvePoints = [];
                                        root.targetFrPoints = [];
                                        root.targetLabel = "";
                                        root.lastRmseText = "";
                                        root.bands = root.fitBandsToDevice(root.presetsMap[name]);
                                        if (root.selectedBand >= root.bands.length) root.selectedBand = 0;
                                        root.dspEnabled = true;
                                        if (root.livePreview && root.activeDevice) root.applyBands(false);
                                    }
                                }
                            }
                            PlasmaComponents3.Button {
                                text: "DEL"
                                font.family: root.monoFont
                                font.pixelSize: 10
                                enabled: presetCombo.currentIndex >= 0
                                onClicked: {
                                    var name = root.presetNames[presetCombo.currentIndex];
                                    root.runBackend("preset", "preset-delete --name-b64 " + root.toB64(name));
                                }
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            PlasmaComponents3.TextField {
                                id: newPresetName
                                Layout.fillWidth: true
                                font.family: root.monoFont
                                font.pixelSize: 11
                                placeholderText: "Guardar preset..."
                            }
                            PlasmaComponents3.Button {
                                text: "SAVE"
                                font.family: root.monoFont
                                font.pixelSize: 10
                                enabled: newPresetName.text.trim().length > 0
                                onClicked: {
                                    root.runBackend(
                                        "preset",
                                        "preset-save --name-b64 " + root.toB64(newPresetName.text.trim()) +
                                        " --bands-b64 " + root.toB64(JSON.stringify(root.bands))
                                    );
                                    newPresetName.text = "";
                                }
                            }
                        }

                        // EqualizerAPO Import / Export Row
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 4
                            PlasmaComponents3.Button {
                                Layout.fillWidth: true
                                text: root.showApoBox ? "CERRAR APO" : "IMPORT APO"
                                font.family: root.monoFont
                                font.pixelSize: 10
                                onClicked: root.showApoBox = !root.showApoBox
                            }
                            PlasmaComponents3.Button {
                                Layout.fillWidth: true
                                text: "EXPORT APO"
                                font.family: root.monoFont
                                font.pixelSize: 10
                                onClicked: root.exportApoText()
                            }
                        }

                        // Inline APO Text Box
                        ColumnLayout {
                            visible: root.showApoBox
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            spacing: 4
                            QQC2.ScrollView {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 68
                                clip: true
                                QQC2.TextArea {
                                    id: apoTextArea
                                    font.family: root.monoFont
                                    font.pixelSize: 10
                                    placeholderText: "Pega ParametricEQ.txt (Filter 1: ON PK Fc 100 Hz Gain 2.5 dB Q 1.2)..."
                                }
                            }
                            PlasmaComponents3.Button {
                                Layout.fillWidth: true
                                text: "APLICAR PARAMETRICEQ.TXT"
                                font.family: root.monoFont
                                font.pixelSize: 10
                                highlighted: true
                                enabled: apoTextArea.text.trim().length > 0
                                onClicked: root.importApoText(apoTextArea.text)
                            }
                        }

                        RowLayout {
                            visible: !!root.activeDevice && !!root.activeDevice.supports_spp && !!root.activeDevice.mac
                            Layout.fillWidth: true
                            PlasmaComponents3.Button {
                                Layout.fillWidth: true
                                text: "SPP READ"
                                font.family: root.monoFont
                                font.pixelSize: 10
                                onClicked: root.runBackend("btspp", "bt-spp --mac " + root.activeDevice.mac + " --action read")
                            }
                            PlasmaComponents3.Button {
                                Layout.fillWidth: true
                                text: "SPP WRITE"
                                font.family: root.monoFont
                                font.pixelSize: 10
                                onClicked: root.runBackend("btspp", "bt-spp --mac " + root.activeDevice.mac + " --action write --bands-b64 " + root.toB64(JSON.stringify(root.bands)))
                            }
                        }

                        Item { Layout.fillHeight: true }
                    }
                }

                // ── Monospace Telemetry Footer ──────────────────────────────
                PlasmaComponents3.Label {
                    Layout.fillWidth: true
                    text: "> " + root.statusMessage
                    font.family: root.monoFont
                    font.pixelSize: 10
                    elide: Text.ElideRight
                    color: root.statusError ? root.coralAccent : root.textMuted
                }
            }
        }
    }
}
