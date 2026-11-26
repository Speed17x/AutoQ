package com.hector.moondropdsp.ui.components

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.hector.moondropdsp.ui.theme.ConsolasFontFamily
import com.hector.moondropdsp.ui.theme.LocalAutoQTheme
import com.hector.moondropdsp.widget.NothingDotMatrixRenderer

@Composable
fun DotMatrixText(
    text: String,
    modifier: Modifier = Modifier,
    color: Color? = null,
    dotSize: Dp = 2.6.dp,
    dotGap: Dp = 1.1.dp,
) {
    val t = LocalAutoQTheme.current
    val resolvedColor = color ?: t.textPrimary
    if (!t.useDotMatrixHeaders) {
        Text(
            text = text,
            modifier = modifier,
            color = resolvedColor,
            fontFamily = ConsolasFontFamily,
            fontWeight = FontWeight.Bold,
            fontSize = 18.sp,
            letterSpacing = 0.6.sp,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        return
    }

    val clean = text.uppercase()
    val stepDp = dotSize + dotGap
    val charWidthDp = stepDp * 6.2f
    val totalWidth = (charWidthDp * clean.length.coerceAtLeast(1)).coerceAtLeast(24.dp)
    val totalHeight = stepDp * 7f

    Canvas(modifier = modifier.width(totalWidth).height(totalHeight)) {
        val stepPx = stepDp.toPx()
        val radiusPx = (dotSize.toPx() * 0.48f)
        val advPx = charWidthDp.toPx()

        clean.forEachIndexed { idx, ch ->
            val glyph = NothingDotMatrixRenderer.getGlyph(ch)
            val x0 = idx * advPx + radiusPx
            for (r in 0 until 7) {
                val row = glyph[r]
                for (c in 0 until 5) {
                    if (row[c] == '#') {
                        drawCircle(
                            color = resolvedColor,
                            radius = radiusPx,
                            center = Offset(x0 + c * stepPx, radiusPx + r * stepPx),
                        )
                    }
                }
            }
        }
    }
}

@Composable
fun NothingCard(
    modifier: Modifier = Modifier,
    cornerRadius: Dp? = null,
    backgroundColor: Color? = null,
    borderColor: Color? = null,
    padding: Dp = 14.dp,
    content: @Composable ColumnScope.() -> Unit,
) {
    val t = LocalAutoQTheme.current
    val rad = cornerRadius ?: t.cardRadiusDp.dp
    val bg = backgroundColor ?: t.surface
    val border = borderColor ?: t.border
    Column(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(rad))
            .background(bg)
            .border(1.dp, border, RoundedCornerShape(rad))
            .padding(padding),
        content = content,
    )
}

@Composable
fun NothingPillChip(
    label: String,
    selected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    accentRed: Boolean = true,
) {
    val t = LocalAutoQTheme.current
    val bg = when {
        selected && accentRed -> t.accent
        selected -> t.textPrimary
        else -> t.surfaceHigh
    }
    val fg = when {
        selected && accentRed -> t.textOnAccent
        selected -> t.background
        else -> t.textMuted
    }
    val border = if (selected) bg else t.border

    Box(
        modifier = modifier
            .clip(CircleShape)
            .background(bg)
            .border(1.dp, border, CircleShape)
            .clickable(onClick = onClick)
            .padding(horizontal = 12.dp, vertical = 7.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = label,
            color = fg,
            style = MaterialTheme.typography.labelMedium,
            fontWeight = FontWeight.Bold,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

@Composable
fun NothingPrimaryButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    accentRed: Boolean = true,
) {
    val t = LocalAutoQTheme.current
    Button(
        onClick = onClick,
        enabled = enabled,
        modifier = modifier.height(42.dp),
        shape = CircleShape,
        colors = ButtonDefaults.buttonColors(
            containerColor = if (accentRed) t.accent else t.textPrimary,
            contentColor = if (accentRed) t.textOnAccent else t.background,
            disabledContainerColor = t.surfaceHigh,
            disabledContentColor = t.textSubtle,
        ),
        contentPadding = PaddingValues(horizontal = 16.dp, vertical = 0.dp),
    ) {
        Text(
            text = if (t.useDotMatrixHeaders) text.uppercase() else text,
            fontFamily = ConsolasFontFamily,
            fontWeight = FontWeight.Bold,
            fontSize = 11.sp,
            letterSpacing = 0.6.sp,
            maxLines = 1,
        )
    }
}

@Composable
fun NothingOutlineButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    accentText: Color? = null,
) {
    val t = LocalAutoQTheme.current
    OutlinedButton(
        onClick = onClick,
        enabled = enabled,
        modifier = modifier.height(42.dp),
        shape = CircleShape,
        border = BorderStroke(1.dp, if (enabled) t.borderStrong else t.border),
        colors = ButtonDefaults.outlinedButtonColors(
            containerColor = t.surfaceHigh,
            contentColor = accentText ?: t.textPrimary,
            disabledContentColor = t.textSubtle,
        ),
        contentPadding = PaddingValues(horizontal = 14.dp, vertical = 0.dp),
    ) {
        Text(
            text = if (t.useDotMatrixHeaders) text.uppercase() else text,
            fontFamily = ConsolasFontFamily,
            fontWeight = FontWeight.Bold,
            fontSize = 11.sp,
            letterSpacing = 0.6.sp,
            maxLines = 1,
        )
    }
}

@Composable
fun NothingStatusDot(active: Boolean, size: Dp = 8.dp) {
    val t = LocalAutoQTheme.current
    Box(
        modifier = Modifier
            .size(size)
            .clip(CircleShape)
            .background(if (active) t.accent else t.textSubtle),
    )
}

@Composable
fun NothingSectionLabel(text: String, trailing: String? = null) {
    val t = LocalAutoQTheme.current
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = text.uppercase(),
            color = t.textMuted,
            style = MaterialTheme.typography.labelSmall,
            letterSpacing = 1.0.sp,
        )
        if (!trailing.isNullOrBlank()) {
            Text(
                text = trailing,
                color = t.textSubtle,
                style = MaterialTheme.typography.labelSmall,
            )
        }
    }
}
