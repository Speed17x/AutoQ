package com.hector.moondropdsp.hardware

import kotlinx.serialization.Serializable

@Serializable
enum class DeviceKind {
    USB_HID,
    BT_AUDIO,
    USB_AUDIO,
    WIRED_AUDIO,
}

@Serializable
enum class DspProtocol {
    KTMICRO_5B,
    MOONDROP_8B,
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
) {
    val vidPid: String
        get() = if (vid != 0 || pid != 0) {
            String.format(java.util.Locale.US, "%04X:%04X", vid, pid)
        } else {
            ""
        }

    val isSystemEq: Boolean
        get() = protocol == DspProtocol.SYSTEM_DYNAMICS
}

object MoondropDeviceCatalog {
    const val KT_VID = 0x31B2
    const val CHU2_DSP_PID = 0x0113

    private data class Moondrop8Spec(
        val matchSubstr: String,
        val displayName: String,
        val shortName: String,
        val maxGain: Double,
        val supportsShelves: Boolean,
        val autoeqHint: String,
    )

    private val MOONDROP8_SPECS = listOf(
        Moondrop8Spec("ag rays", "Moondrop AG Rays", "AG RAYS", 12.0, false, "Moondrop"),
        Moondrop8Spec("rays", "Moondrop Rays", "RAYS DSP", 12.0, false, "Moondrop"),
        Moondrop8Spec("marigold", "Moondrop Marigold", "MARIGOLD", 6.0, false, "Moondrop"),
        Moondrop8Spec("freedsp pro", "Moondrop FreeDSP Pro", "FREEDSP PRO", 12.0, true, "Moondrop"),
        Moondrop8Spec("freedsp mini", "Moondrop FreeDSP Mini", "FREEDSP MINI", 12.0, true, "Moondrop"),
        Moondrop8Spec("dawn pro2", "Moondrop DAWN PRO 2", "DAWN PRO 2", 12.0, true, "Moondrop"),
        Moondrop8Spec("echo a", "Moondrop Echo A", "ECHO A", 12.0, false, "Moondrop"),
        Moondrop8Spec("moonriver 3", "Moondrop MOONRIVER 3", "MOONRIVER 3", 12.0, true, "Moondrop"),
        Moondrop8Spec("dha15", "Moondrop DHA15", "DHA15", 12.0, true, "Moondrop"),
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
        "space travel 2" to "Moondrop Space Travel 2",
        "space travel" to "Moondrop Space Travel",
        "golden ages 2" to "Moondrop Golden Ages",
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
                "tanchjim" in low ->
                    Triple(productName.ifBlank { "Tanchjim DSP" }, shortenWidgetName(productName), productName)
                else ->
                    Triple(productName.ifBlank { "USB DSP (KTMicro)" }, shortenWidgetName(productName.ifBlank { "USB DSP" }), productName.ifBlank { "Moondrop" })
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
            )
        }

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
                )
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
        // Clean common Bluetooth suffixes so AutoEQ search matches Sony, AirPods, Nothing, Huawei, Buds, etc.
        return name
            .replace(Regex("(?i)\\b(le|ble|hands-free|ag|audio|stereo|headset|headphones|earbuds)\\b"), "")
            .replace(Regex("\\s+"), " ")
            .trim()
    }

    fun shortenWidgetName(displayName: String): String {
        val cleaned = displayName
            .replace(Regex("(?i)^(moondrop|huawei|sony|samsung|apple)\\s+"), "")
            .trim()
            .uppercase()
        return cleaned.ifBlank { displayName.uppercase().ifBlank { "AUTOQ" } }.take(14)
    }
}
