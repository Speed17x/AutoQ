package com.hector.moondropdsp.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.staticCompositionLocalOf

val LocalAutoQTheme = staticCompositionLocalOf { PureHyperTokens }

@Composable
fun MoondropDSPTheme(
    themeId: AppThemeId = AppThemeId.PURE_HYPER,
    content: @Composable () -> Unit,
) {
    val tokens = tokensFor(themeId)
    val scheme = if (tokens.isDark) {
        darkColorScheme(
            primary = tokens.accent,
            onPrimary = tokens.textOnAccent,
            primaryContainer = tokens.accentDim,
            onPrimaryContainer = tokens.textPrimary,
            secondary = tokens.textPrimary,
            onSecondary = tokens.background,
            secondaryContainer = tokens.surfaceHigh,
            onSecondaryContainer = tokens.textPrimary,
            tertiary = tokens.emerald,
            onTertiary = tokens.background,
            background = tokens.background,
            onBackground = tokens.textPrimary,
            surface = tokens.surface,
            onSurface = tokens.textPrimary,
            surfaceVariant = tokens.surfaceHigh,
            onSurfaceVariant = tokens.textMuted,
            outline = tokens.border,
            outlineVariant = tokens.border,
            error = tokens.accent,
            onError = tokens.textOnAccent,
        )
    } else {
        lightColorScheme(
            primary = tokens.accent,
            onPrimary = tokens.textOnAccent,
            primaryContainer = tokens.accentDim,
            onPrimaryContainer = tokens.textPrimary,
            secondary = tokens.textPrimary,
            onSecondary = tokens.surface,
            secondaryContainer = tokens.surfaceHigh,
            onSecondaryContainer = tokens.textPrimary,
            tertiary = tokens.emerald,
            onTertiary = tokens.textOnAccent,
            background = tokens.background,
            onBackground = tokens.textPrimary,
            surface = tokens.surface,
            onSurface = tokens.textPrimary,
            surfaceVariant = tokens.surfaceHigh,
            onSurfaceVariant = tokens.textMuted,
            outline = tokens.border,
            outlineVariant = tokens.border,
            error = tokens.accent,
            onError = tokens.textOnAccent,
        )
    }

    CompositionLocalProvider(LocalAutoQTheme provides tokens) {
        MaterialTheme(
            colorScheme = scheme,
            typography = Typography,
            content = content,
        )
    }
}
