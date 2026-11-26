package com.hector.moondropdsp.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable

private val NothingDarkColorScheme = darkColorScheme(
    primary = NothingRed,
    onPrimary = NothingWhite,
    primaryContainer = NothingRedDim,
    onPrimaryContainer = NothingWhite,
    secondary = NothingWhite,
    onSecondary = NothingObsidian,
    secondaryContainer = NothingSurfaceHigh,
    onSecondaryContainer = NothingWhite,
    tertiary = NothingEmerald,
    onTertiary = NothingObsidian,
    background = NothingObsidian,
    onBackground = NothingWhite,
    surface = NothingSurface,
    onSurface = NothingWhite,
    surfaceVariant = NothingSurfaceHigh,
    onSurfaceVariant = NothingMuted,
    outline = NothingBorder,
    outlineVariant = NothingBorder,
    error = NothingRed,
    onError = NothingWhite,
)

@Composable
fun MoondropDSPTheme(
    content: @Composable () -> Unit,
) {
    MaterialTheme(
        colorScheme = NothingDarkColorScheme,
        typography = Typography,
        content = content,
    )
}
