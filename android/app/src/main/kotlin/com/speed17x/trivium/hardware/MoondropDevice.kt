package com.speed17x.trivium.hardware

import kotlinx.serialization.Serializable
import java.util.Locale

@Serializable
enum class DeviceKind {
    USB_HID,
    BLE_GATT,
    BT_AUDIO,
    USB_AUDIO,
    WIRED_AUDIO,
}

@Serializable
enum class DspProtocol {
    KTMICRO_5B,
    MOONDROP_8B,
    CONEXANT_9B,
    WALKPLAY,
    FIIO,
    QUDELIX_5K,
    TOPPING,
    FOSI_DS3,
    JDS_ELEMENT,
    FIIO_BLE,
    AIROHA_BLE,
    SYSTEM_DYNAMICS,
}

@Serializable
data class MoondropDevice(
    val id: String,
    val kind: DeviceKind,
    val protocol: DspProtocol,
    val displayName: String,
    val shortWidgetName: String,
    val transportBadge: String,
    val maxBands: Int,
    val maxGainDb: Double = 12.0,
    val supportsShelves: Boolean = true,
    val disconnectsOnCommit: Boolean = false,
    val autoeqHint: String = "",
    val vid: Int = 0,
    val pid: Int = 0,
    val usbDeviceName: String = "",
    val btMac: String = "",
    val hasUsbPermission: Boolean = true,
    val supportsMoondropSpp: Boolean = false,
    val reportId: Int = 0x4B,
    val userSlot: Int = 7,
    val bypassSlot: Int = 0,
    val fiioSaveV2: Boolean = false,
) {
    val vidPid: String
        get() = if (vid != 0 || pid != 0) {
            String.format(Locale.US, "%04X:%04X", vid, pid)
        } else {
            ""
        }

    val isSystemEq: Boolean
        get() = protocol == DspProtocol.SYSTEM_DYNAMICS
}

object MoondropDeviceCatalog {
    const val KT_VID = 0x31B2
    const val CHU2_DSP_PID = 0x0113

    const val CONEXANT_VID = 0x35D8
    const val CONEXANT_FREEDSP_PID = 0x1496
    const val CONEXANT_ECHOB_PID = 0x149B

    const val FIIO_VID_PRIMARY = 0x2972
    const val FIIO_VID_QUALCOMM = 0x0A12
    const val THESYCON_SHARED_VID = 0x152A
    const val JDS_ELEMENT_IV_PID = 0x88FA

    val WALKPLAY_VIDS = setOf(
        0x0104, 0x011B, 0x011D, 0x0661, 0x0663, 0x0666, 0x0762, 0x0909, 0x0D8C, 0x2FC6, 0x3302,
        0x34BE, 0x35D8, 0x36A7, 0x373B, 0x60C1, 0x60E1, 0xB445, 0xB44D,
    )

    private data class Moondrop8Spec(
        val matchSubstr: String,
        val displayName: String,
        val shortName: String,
        val maxGain: Double,
        val supportsShelves: Boolean,
        val autoeqHint: String,
    )

    private data class FiioSpec(
        val matchSubstr: String,
        val displayName: String,
        val shortName: String,
        val maxBands: Int,
        val maxGain: Double,
        val reportId: Int,
        val userSlot: Int,
        val bypassSlot: Int,
        val disconnectsOnCommit: Boolean,
        val saveV2: Boolean,
        val hint: String,
    )

    private data class WalkPlaySpec(
        val matchSubstr: String,
        val displayName: String,
        val shortName: String,
        val maxBands: Int,
        val maxGain: Double,
        val supportsShelves: Boolean,
        val slot: Int,
        val hint: String,
    )

    private val MOONDROP8_SPECS = listOf(
        Moondrop8Spec("ag rays", "Moondrop AG Rays", "AG RAYS", 12.0, false, "Moondrop"),
        Moondrop8Spec("rays", "Moondrop Rays", "RAYS DSP", 12.0, false, "Moondrop"),
        Moondrop8Spec("marigold", "Moondrop Marigold", "MARIGOLD", 6.0, false, "Moondrop"),
        Moondrop8Spec("freedsp pro", "Moondrop FreeDSP Pro", "FREEDSP PRO", 12.0, true, "Moondrop"),
        Moondrop8Spec("freedsp mini", "Moondrop FreeDSP Mini", "FREEDSP MINI", 12.0, true, "Moondrop"),
        Moondrop8Spec("dawn pro2", "Moondrop DAWN PRO 2", "DAWN PRO 2", 12.0, true, "Moondrop"),
        Moondrop8Spec("dawn pro 2", "Moondrop DAWN PRO 2", "DAWN PRO 2", 12.0, true, "Moondrop"),
        Moondrop8Spec("echo a", "Moondrop Echo A", "ECHO A", 12.0, false, "Moondrop"),
        Moondrop8Spec("moonriver 3", "Moondrop MOONRIVER 3", "MOONRIVER 3", 12.0, true, "Moondrop"),
        Moondrop8Spec("dha15", "Moondrop DHA15", "DHA15", 12.0, true, "Moondrop"),
    )

    private val FIIO_SPECS = listOf(
        FiioSpec("ja11", "JadeAudio JA11", "JADE JA11", 5, 12.0, 2, 3, 4, true, false, "JadeAudio JA11"),
        FiioSpec("jiezi", "JadeAudio JIEZI", "JIEZI", 5, 12.0, 2, 160, 240, false, false, "JadeAudio"),
        FiioSpec("ka17", "FiiO KA17", "FIIO KA17", 10, 12.0, 1, 7, 10, false, false, "FiiO"),
        FiioSpec("ka15", "FiiO KA15", "FIIO KA15", 10, 12.0, 7, 160, 10, false, false, "FiiO"),
        FiioSpec("qx13", "FiiO QX13", "FIIO QX13", 10, 12.0, 7, 160, 240, false, false, "FiiO"),
        FiioSpec("btr17", "FiiO BTR17", "FIIO BTR17", 10, 12.0, 7, 160, 240, false, true, "FiiO"),
        FiioSpec("btr13", "FiiO BTR13", "FIIO BTR13", 10, 12.0, 7, 160, 240, false, false, "FiiO"),
        FiioSpec("bt11", "FiiO BT11", "FIIO BT11", 10, 12.0, 7, 7, 11, false, false, "FiiO"),
        FiioSpec("q7", "FiiO Q7", "FIIO Q7", 10, 12.0, 7, 7, 10, false, false, "FiiO"),
        FiioSpec("snowsky melody", "SNOWSKY Melody", "MELODY", 10, 12.0, 7, 160, 240, false, false, "FiiO"),
        FiioSpec("snowsky tiny", "SNOWSKY TINY", "TINY", 10, 12.0, 7, 160, 240, false, false, "FiiO"),
    )

    private val WALKPLAY_SPECS = listOf(
        WalkPlaySpec("protocol max", "CrinEar Protocol Max", "PROTOCOL MAX", 10, 10.0, true, 101, "CrinEar"),
        WalkPlaySpec("stargate", "Tanchjim Stargate II", "STARGATE II", 8, 12.0, true, 101, "Tanchjim"),
        WalkPlaySpec("bunny", "Tanchjim Bunny DSP", "BUNNY DSP", 5, 12.0, true, 101, "Tanchjim Bunny"),
        WalkPlaySpec("space pro", "Tanchjim Space Pro", "SPACE PRO", 8, 12.0, true, 101, "Tanchjim"),
        WalkPlaySpec("tanchjim", "Tanchjim USB DSP", "TANCHJIM", 5, 12.0, true, 101, "Tanchjim"),
        WalkPlaySpec("nicehck", "NiceHCK USB DSP", "NICEHCK", 8, 12.0, true, 101, "NiceHCK"),
        WalkPlaySpec("letshuoer", "LETSHUOER USB DSP", "LETSHUOER", 8, 12.0, true, 101, "LETSHUOER"),
        WalkPlaySpec("ddhifi", "ddHifi USB DSP", "DDHIFI DSP", 8, 12.0, true, 101, "ddHifi"),
        WalkPlaySpec("truthear", "Truthear USB DSP", "TRUTHEAR", 8, 12.0, true, 101, "Truthear"),
        WalkPlaySpec("walkplay", "WalkPlay USB DSP", "WALKPLAY", 8, 10.0, true, 101, ""),
    )

    private val MOONDROP_BT_KEYWORDS = listOf(
        "moondrop",
        "space travel",
        "golden ages",
        "nekocake",
        "moca",
        "robin",
        "marigold",
        "kuroneko",
        "ultrasonic",
        "pill",
        "edge",
        "block",
        "alice",
    )

    private val BT_AUTOEQ_HINTS = listOf(
        "space travel 2 ultra" to "Moondrop Space Travel 2 Ultra",
        "space travel 2" to "Moondrop Space Travel 2",
        "space travel" to "Moondrop Space Travel",
        "golden ages 2" to "Moondrop Golden Ages 2",
        "golden ages" to "Moondrop Golden Ages",
        "moca" to "Moondrop Moca",
        "nekocake" to "Moondrop Nekocake",
        "edge" to "Moondrop Edge",
        "block" to "Moondrop BLOCK",
        "alice" to "Moondrop Alice",
    )

    fun classifyUsb(
        deviceName: String,
        vid: Int,
        pid: Int,
        productName: String,
        hasPermission: Boolean,
    ): MoondropDevice? {
        val low = productName.lowercase()

        // 1. KTMicro 5-band family (VID 0x31B2): Moondrop, Kiwi Ears, JCALLY, Tanchjim
        if (vid == KT_VID) {
            val (display, shortName, hint) = when {
                pid == CHU2_DSP_PID || "chu2" in low || "chu 2" in low ->
                    Triple("Moondrop Chu 2 DSP", "CHU 2 DSP", "Moondrop Chu 2")
                "quark" in low ->
                    Triple("Moondrop Quark 2 DSP", "QUARK 2", "Moondrop Quark")
                "jiu" in low ->
                    Triple("Moondrop Jiu DSP", "JIU DSP", "Moondrop Jiu")
                "may" in low ->
                    Triple("Moondrop May DSP", "MAY DSP", "Moondrop May")
                "kiwi" in low || "allegro" in low ->
                    Triple(productName.ifBlank { "Kiwi Ears Allegro DSP" }, shortenWidgetName(productName.ifBlank { "ALLEGRO DSP" }), "Kiwi Ears")
                "jcally" in low || "jm12" in low ->
                    Triple(productName.ifBlank { "JCALLY USB DSP" }, shortenWidgetName(productName.ifBlank { "JCALLY DSP" }), "")
                "tanchjim" in low || "tanya" in low || "zero" in low ->
                    Triple(productName.ifBlank { "Tanchjim USB DSP" }, shortenWidgetName(productName.ifBlank { "TANCHJIM" }), "Tanchjim")
                else ->
                    Triple(productName.ifBlank { "KTMicro DSP" }, shortenWidgetName(productName.ifBlank { "KTMICRO DSP" }), if ("moondrop" in low) "Moondrop" else "")
            }
            return MoondropDevice(
                id = "usb:$deviceName",
                kind = DeviceKind.USB_HID,
                protocol = DspProtocol.KTMICRO_5B,
                displayName = display,
                shortWidgetName = shortName,
                transportBadge = "HID·5B",
                maxBands = 5,
                maxGainDb = 12.0,
                supportsShelves = true,
                disconnectsOnCommit = true,
                autoeqHint = hint,
                vid = vid,
                pid = pid,
                usbDeviceName = deviceName,
                hasUsbPermission = hasPermission,
                reportId = 0x4B,
                userSlot = 3,
                bypassSlot = 2,
            )
        }

        // 2. Moondrop 8-band HID devices
        for (spec in MOONDROP8_SPECS) {
            if (spec.matchSubstr in low) {
                return MoondropDevice(
                    id = "usb:$deviceName",
                    kind = DeviceKind.USB_HID,
                    protocol = DspProtocol.MOONDROP_8B,
                    displayName = spec.displayName,
                    shortWidgetName = spec.shortName,
                    transportBadge = "HID·8B",
                    maxBands = 8,
                    maxGainDb = spec.maxGain,
                    supportsShelves = spec.supportsShelves,
                    disconnectsOnCommit = false,
                    autoeqHint = spec.autoeqHint,
                    vid = vid,
                    pid = pid,
                    usbDeviceName = deviceName,
                    hasUsbPermission = hasPermission,
                    reportId = 0x4B,
                    userSlot = 7,
                    bypassSlot = 0,
                )
            }
        }

        // 3. Conexant Freeman 9-band HID (Moondrop FreeDSP & ECHO-B; check before WalkPlay)
        if (vid == CONEXANT_VID &&
            (pid == CONEXANT_FREEDSP_PID ||
                pid == CONEXANT_ECHOB_PID ||
                "echo-b" in low ||
                "echo b" in low ||
                ("freedsp" in low && "pro" !in low && "mini" !in low))
        ) {
            val isEchoB = pid == CONEXANT_ECHOB_PID || "echo-b" in low || "echo b" in low
            return MoondropDevice(
                id = "usb:$deviceName",
                kind = DeviceKind.USB_HID,
                protocol = DspProtocol.CONEXANT_9B,
                displayName = if (isEchoB) "Moondrop ECHO-B" else "Moondrop FreeDSP",
                shortWidgetName = if (isEchoB) "ECHO-B" else "FREEDSP",
                transportBadge = "CX·9B",
                maxBands = 9,
                maxGainDb = 12.0,
                supportsShelves = true,
                disconnectsOnCommit = false,
                autoeqHint = "Moondrop",
                vid = vid,
                pid = pid,
                usbDeviceName = deviceName,
                hasUsbPermission = hasPermission,
                reportId = 0x01,
                userSlot = 0,
                bypassSlot = 0,
            )
        }

        // 4. Qudelix-5K / 5K+ (VID 0x0A12 shared with Qualcomm; check before FiiO)
        if (vid == FIIO_VID_QUALCOMM && ("qudelix" in low || "5k" in low || pid in setOf(0x4124, 0x4125, 0x4003))) {
            val disp = productName.trim().ifBlank { "Qudelix-5K" }
            return MoondropDevice(
                id = "usb:$deviceName",
                kind = DeviceKind.USB_HID,
                protocol = DspProtocol.QUDELIX_5K,
                displayName = disp,
                shortWidgetName = "QUDELIX 5K",
                transportBadge = "Q5K·10B",
                maxBands = 10,
                maxGainDb = 12.0,
                supportsShelves = true,
                disconnectsOnCommit = false,
                autoeqHint = "Qudelix",
                vid = vid,
                pid = pid,
                usbDeviceName = deviceName,
                hasUsbPermission = hasPermission,
                reportId = 8,
                userSlot = 0,
                bypassSlot = 0,
            )
        }

        // 4. Shared Thesycon VID 0x152A (JDS Labs Element IV CDC, Topping PEQ, Fosi Audio DS3)
        if (vid == THESYCON_SHARED_VID) {
            if (pid == JDS_ELEMENT_IV_PID || "element iv" in low) {
                val disp = productName.trim().ifBlank { "JDS Labs Element IV" }
                return MoondropDevice(
                    id = "usb:$deviceName",
                    kind = DeviceKind.USB_HID,
                    protocol = DspProtocol.JDS_ELEMENT,
                    displayName = disp,
                    shortWidgetName = "ELEMENT IV",
                    transportBadge = "JDS·12B",
                    maxBands = 12,
                    maxGainDb = 12.0,
                    supportsShelves = true,
                    disconnectsOnCommit = false,
                    autoeqHint = "",
                    vid = vid,
                    pid = pid,
                    usbDeviceName = deviceName,
                    hasUsbPermission = hasPermission,
                    reportId = 0,
                    userSlot = 0,
                    bypassSlot = 0,
                )
            }
            if ("topping" in low || "dx1 ii" in low || "e50 ii" in low || "d50 iii" in low || "dx5 ii" in low) {
                val disp = productName.trim().ifBlank { "Topping PEQ" }
                return MoondropDevice(
                    id = "usb:$deviceName",
                    kind = DeviceKind.USB_HID,
                    protocol = DspProtocol.TOPPING,
                    displayName = disp,
                    shortWidgetName = shortenWidgetName(disp),
                    transportBadge = "TOP·11B",
                    maxBands = 11,
                    maxGainDb = 12.0,
                    supportsShelves = true,
                    disconnectsOnCommit = false,
                    autoeqHint = "Topping",
                    vid = vid,
                    pid = pid,
                    usbDeviceName = deviceName,
                    hasUsbPermission = hasPermission,
                    reportId = 0,
                    userSlot = 0,
                    bypassSlot = 0,
                )
            }
            if ("fosi" in low || "ds3" in low || "zh3" in low || "k7" in low) {
                val disp = productName.trim().ifBlank { "Fosi Audio DS3" }
                return MoondropDevice(
                    id = "usb:$deviceName",
                    kind = DeviceKind.USB_HID,
                    protocol = DspProtocol.FOSI_DS3,
                    displayName = disp,
                    shortWidgetName = "FOSI DS3",
                    transportBadge = "FOSI·8B",
                    maxBands = 8,
                    maxGainDb = 12.0,
                    supportsShelves = true,
                    disconnectsOnCommit = false,
                    autoeqHint = "Fosi",
                    vid = vid,
                    pid = pid,
                    usbDeviceName = deviceName,
                    hasUsbPermission = hasPermission,
                    reportId = 1,
                    userSlot = 7,
                    bypassSlot = 0,
                )
            }
        }

        // 5. FiiO / JadeAudio / SNOWSKY USB HID PEQ family
        if (vid == FIIO_VID_PRIMARY || (vid == FIIO_VID_QUALCOMM && ("fiio" in low || "jade" in low || "snowsky" in low))) {
            for (spec in FIIO_SPECS) {
                if (spec.matchSubstr in low) {
                    return MoondropDevice(
                        id = "usb:$deviceName",
                        kind = DeviceKind.USB_HID,
                        protocol = DspProtocol.FIIO,
                        displayName = spec.displayName,
                        shortWidgetName = spec.shortName,
                        transportBadge = "FIIO·${spec.maxBands}B",
                        maxBands = spec.maxBands,
                        maxGainDb = spec.maxGain,
                        supportsShelves = true,
                        disconnectsOnCommit = spec.disconnectsOnCommit,
                        autoeqHint = spec.hint,
                        vid = vid,
                        pid = pid,
                        usbDeviceName = deviceName,
                        hasUsbPermission = hasPermission,
                        reportId = spec.reportId,
                        userSlot = spec.userSlot,
                        bypassSlot = spec.bypassSlot,
                        fiioSaveV2 = spec.saveV2,
                    )
                }
            }
            if (vid == FIIO_VID_PRIMARY) {
                val disp = productName.trim().ifBlank { "FiiO USB DSP" }
                return MoondropDevice(
                    id = "usb:$deviceName",
                    kind = DeviceKind.USB_HID,
                    protocol = DspProtocol.FIIO,
                    displayName = disp,
                    shortWidgetName = shortenWidgetName(disp),
                    transportBadge = "FIIO·10B",
                    maxBands = 10,
                    maxGainDb = 12.0,
                    supportsShelves = true,
                    disconnectsOnCommit = false,
                    autoeqHint = "FiiO",
                    vid = vid,
                    pid = pid,
                    usbDeviceName = deviceName,
                    hasUsbPermission = hasPermission,
                    reportId = 7,
                    userSlot = 160,
                    bypassSlot = 240,
                )
            }
        }

        // 6. WalkPlay DSP chipset family (19 vendor IDs)
        if (vid in WALKPLAY_VIDS) {
            if (vid == 0x3302 && pid == 0x43CC) {
                return MoondropDevice(
                    id = "usb:$deviceName",
                    kind = DeviceKind.USB_HID,
                    protocol = DspProtocol.WALKPLAY,
                    displayName = "CrinEar Protocol Max",
                    shortWidgetName = "PROTOCOL MAX",
                    transportBadge = "WP·10B",
                    maxBands = 10,
                    maxGainDb = 10.0,
                    supportsShelves = true,
                    disconnectsOnCommit = false,
                    autoeqHint = "CrinEar",
                    vid = vid,
                    pid = pid,
                    usbDeviceName = deviceName,
                    hasUsbPermission = hasPermission,
                    reportId = 0x4B,
                    userSlot = 101,
                    bypassSlot = 0,
                )
            }
            for (spec in WALKPLAY_SPECS) {
                if (spec.matchSubstr in low) {
                    return MoondropDevice(
                        id = "usb:$deviceName",
                        kind = DeviceKind.USB_HID,
                        protocol = DspProtocol.WALKPLAY,
                        displayName = spec.displayName,
                        shortWidgetName = spec.shortName,
                        transportBadge = "WP·${spec.maxBands}B",
                        maxBands = spec.maxBands,
                        maxGainDb = spec.maxGain,
                        supportsShelves = spec.supportsShelves,
                        disconnectsOnCommit = false,
                        autoeqHint = spec.hint,
                        vid = vid,
                        pid = pid,
                        usbDeviceName = deviceName,
                        hasUsbPermission = hasPermission,
                        reportId = 0x4B,
                        userSlot = spec.slot,
                        bypassSlot = 0,
                    )
                }
            }
        }

        return null
    }

    fun isMoondropBluetoothName(name: String): Boolean {
        val low = name.lowercase()
        return MOONDROP_BT_KEYWORDS.any { it in low }
    }

    fun inferAutoEqHint(name: String): String {
        val low = name.lowercase()
        for ((substr, hint) in BT_AUTOEQ_HINTS) {
            if (substr in low) return hint
        }
        return name
            .replace(Regex("(?i)\\b(le|ble|hands-free|ag|audio|stereo|headset|headphones|earbuds)\\b"), "")
            .replace(Regex("\\s+"), " ")
            .trim()
    }

    fun classifyBleDevice(name: String, address: String): MoondropDevice? {
        val low = name.lowercase().trim()
        if (low.startsWith("fiio eh11") || low.startsWith("fiio eh13") || low.contains("eh11") || low.contains("eh13")) {
            val disp = name.trim().ifBlank { "FiiO BLE DSP" }
            return MoondropDevice(
                id = "ble:$address",
                kind = DeviceKind.BLE_GATT,
                protocol = DspProtocol.FIIO_BLE,
                displayName = disp,
                shortWidgetName = shortenWidgetName(disp),
                transportBadge = "BLE·10B",
                maxBands = 10,
                maxGainDb = 20.0,
                supportsShelves = true,
                disconnectsOnCommit = false,
                autoeqHint = disp,
                btMac = address,
            )
        }
        if ("audeze maxwell" in low || low == "maxwell" || "maxwell" in low) {
            return MoondropDevice(
                id = "ble:$address",
                kind = DeviceKind.BLE_GATT,
                protocol = DspProtocol.AIROHA_BLE,
                displayName = name.trim().ifBlank { "Audeze Maxwell BLE" },
                shortWidgetName = "MAXWELL",
                transportBadge = "BLE·10B",
                maxBands = 10,
                maxGainDb = 12.0,
                supportsShelves = true,
                disconnectsOnCommit = false,
                autoeqHint = "Audeze Maxwell",
                btMac = address,
            )
        }
        return null
    }

    fun shortenWidgetName(displayName: String): String {
        val cleaned = displayName
            .replace(Regex("(?i)^(moondrop|huawei|sony|samsung|apple|fiio|topping|fosi audio)\\s+"), "")
            .trim()
            .uppercase()
        return cleaned.ifBlank { displayName.uppercase().ifBlank { "TRIVIUM" } }.take(14)
    }
}
