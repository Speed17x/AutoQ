package com.hector.moondropdsp.ui.theme

import androidx.compose.ui.graphics.Color

enum class AppThemeId(
    val key: String,
    val label: String,
    val subtitle: String,
) {
    PURE_HYPER("pure_hyper", "Android / HyperOS", "Oscuro limpio estilo HyperOS + Consolas"),
    PURE_LIGHT("pure_light", "HyperOS Claro", "Material claro + azul HyperOS + Consolas"),
    NOTHING_OS("nothing_os", "Nothing OS", "Obsidiana + Matriz de puntos + Rojo");

    companion object {
        fun fromKey(key: String?): AppThemeId =
            entries.firstOrNull { it.key == key } ?: PURE_HYPER
    }
}

data class AutoQThemeTokens(
    val id: AppThemeId,
    val isDark: Boolean,
    val background: Color,
    val surface: Color,
    val surfaceHigh: Color,
    val graphBg: Color,
    val insetBg: Color,
    val border: Color,
    val borderStrong: Color,
    val accent: Color,
    val accentDim: Color,
    val textPrimary: Color,
    val textOnAccent: Color,
    val textMuted: Color,
    val textSubtle: Color,
    val gridColor: Color,
    val zeroLineColor: Color,
    val stockLineColor: Color,
    val targetLineColor: Color,
    val emerald: Color,
    val amber: Color,
    val cardRadiusDp: Int,
    val useDotMatrixHeaders: Boolean,
    val useDotMatrixGrid: Boolean,
)

val PureHyperTokens = AutoQThemeTokens(
    id = AppThemeId.PURE_HYPER,
    isDark = true,
    background = Color(0xFF0E1116),
    surface = Color(0xFF171B22),
    surfaceHigh = Color(0xFF1F2530),
    graphBg = Color(0xFF12151C),
    insetBg = Color(0xFF12151B),
    border = Color(0xFF28303D),
    borderStrong = Color(0xFF3D4A5E),
    accent = Color(0xFF3482FF),
    accentDim = Color(0x333482FF),
    textPrimary = Color(0xFFF2F5FA),
    textOnAccent = Color(0xFFFFFFFF),
    textMuted = Color(0xFF949FB2),
    textSubtle = Color(0xFF636E80),
    gridColor = Color(0xFF232A36),
    zeroLineColor = Color(0xFF394354),
    stockLineColor = Color(0xFF7B889B),
    targetLineColor = Color(0xFFD8E2F2),
    emerald = Color(0xFF1CDC9A),
    amber = Color(0xFFF5B942),
    cardRadiusDp = 22,
    useDotMatrixHeaders = false,
    useDotMatrixGrid = false,
)

val PureLightTokens = AutoQThemeTokens(
    id = AppThemeId.PURE_LIGHT,
    isDark = false,
    background = Color(0xFFF4F6FA),
    surface = Color(0xFFFFFFFF),
    surfaceHigh = Color(0xFFEEF2F8),
    graphBg = Color(0xFFF9FAFC),
    insetBg = Color(0xFFF0F3F8),
    border = Color(0xFFD5DCE6),
    borderStrong = Color(0xFFADBACC),
    accent = Color(0xFF2563EB),
    accentDim = Color(0x282563EB),
    textPrimary = Color(0xFF111827),
    textOnAccent = Color(0xFFFFFFFF),
    textMuted = Color(0xFF525E71),
    textSubtle = Color(0xFF7A8699),
    gridColor = Color(0xFFE2E8F0),
    zeroLineColor = Color(0xFFB8C4D6),
    stockLineColor = Color(0xFF64748B),
    targetLineColor = Color(0xFF1E293B),
    emerald = Color(0xFF0D9488),
    amber = Color(0xFFD97706),
    cardRadiusDp = 22,
    useDotMatrixHeaders = false,
    useDotMatrixGrid = false,
)

val NothingOsTokens = AutoQThemeTokens(
    id = AppThemeId.NOTHING_OS,
    isDark = true,
    background = Color(0xFF0B0C0E),
    surface = Color(0xFF141517),
    surfaceHigh = Color(0xFF1C1E22),
    graphBg = Color(0xFF0E0F12),
    insetBg = Color(0xFF121316),
    border = Color(0xFF282B32),
    borderStrong = Color(0xFF3A3E48),
    accent = Color(0xFFD71921),
    accentDim = Color(0x33D71921),
    textPrimary = Color(0xFFF5F5F5),
    textOnAccent = Color(0xFFFFFFFF),
    textMuted = Color(0xFF8E929B),
    textSubtle = Color(0xFF565A64),
    gridColor = Color(0xFF282B33),
    zeroLineColor = Color(0xFF3A3E48),
    stockLineColor = Color(0xFF6B707B),
    targetLineColor = Color(0xFFE8ECF2),
    emerald = Color(0xFF1CDC9A),
    amber = Color(0xFFF5B942),
    cardRadiusDp = 22,
    useDotMatrixHeaders = true,
    useDotMatrixGrid = true,
)

fun tokensFor(id: AppThemeId): AutoQThemeTokens = when (id) {
    AppThemeId.PURE_HYPER -> PureHyperTokens
    AppThemeId.PURE_LIGHT -> PureLightTokens
    AppThemeId.NOTHING_OS -> NothingOsTokens
}

// Compatibility aliases used during transition/defaults
val NothingObsidian = PureHyperTokens.background
val NothingSurface = PureHyperTokens.surface
val NothingSurfaceHigh = PureHyperTokens.surfaceHigh
val NothingGraphBg = PureHyperTokens.graphBg
val NothingBorder = PureHyperTokens.border
val NothingBorderStrong = PureHyperTokens.borderStrong
val NothingRed = Color(0xFFD71921)
val NothingRedDim = Color(0x33D71921)
val NothingWhite = PureHyperTokens.textPrimary
val NothingMuted = PureHyperTokens.textMuted
val NothingSubtle = PureHyperTokens.textSubtle
val NothingEmerald = PureHyperTokens.emerald
val NothingAmber = PureHyperTokens.amber
