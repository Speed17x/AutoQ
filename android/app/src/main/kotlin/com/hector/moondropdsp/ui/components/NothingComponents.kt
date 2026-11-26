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
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.hector.moondropdsp.ui.theme.NothingBorder
import com.hector.moondropdsp.ui.theme.NothingBorderStrong
import com.hector.moondropdsp.ui.theme.NothingMuted
import com.hector.moondropdsp.ui.theme.NothingRed
import com.hector.moondropdsp.ui.theme.NothingSubtle
import com.hector.moondropdsp.ui.theme.NothingSurface
import com.hector.moondropdsp.ui.theme.NothingSurfaceHigh
import com.hector.moondropdsp.ui.theme.NothingWhite
import com.hector.moondropdsp.widget.NothingDotMatrixRenderer

@Composable
fun DotMatrixText(
    text: String,
    modifier: Modifier = Modifier,
    color: Color = NothingWhite,
    dotSize: Dp = 2.6.dp,
    dotGap: Dp = 1.1.dp,
) {
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
                            color = color,
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
    cornerRadius: Dp = 22.dp,
    backgroundColor: Color = NothingSurface,
    borderColor: Color = NothingBorder,
    padding: Dp = 14.dp,
    content: @Composable ColumnScope.() -> Unit,
) {
    Column(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(cornerRadius))
            .background(backgroundColor)
            .border(1.dp, borderColor, RoundedCornerShape(cornerRadius))
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
    val bg = when {
        selected && accentRed -> NothingRed
        selected -> NothingWhite
        else -> NothingSurfaceHigh
    }
    val fg = when {
        selected && accentRed -> NothingWhite
        selected -> Color(0xFF0B0C0E)
        else -> NothingMuted
    }
    val border = if (selected) bg else NothingBorder

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
    Button(
        onClick = onClick,
        enabled = enabled,
        modifier = modifier.height(42.dp),
        shape = CircleShape,
        colors = ButtonDefaults.buttonColors(
            containerColor = if (accentRed) NothingRed else NothingWhite,
            contentColor = if (accentRed) NothingWhite else Color(0xFF0B0C0E),
            disabledContainerColor = NothingSurfaceHigh,
            disabledContentColor = NothingSubtle,
        ),
        contentPadding = PaddingValues(horizontal = 16.dp, vertical = 0.dp),
    ) {
        Text(
            text = text.uppercase(),
            fontFamily = FontFamily.Monospace,
            fontWeight = FontWeight.Bold,
            fontSize = 11.sp,
            letterSpacing = 0.8.sp,
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
    accentText: Color = NothingWhite,
) {
    OutlinedButton(
        onClick = onClick,
        enabled = enabled,
        modifier = modifier.height(42.dp),
        shape = CircleShape,
        border = BorderStroke(1.dp, if (enabled) NothingBorderStrong else NothingBorder),
        colors = ButtonDefaults.outlinedButtonColors(
            containerColor = NothingSurfaceHigh,
            contentColor = accentText,
            disabledContentColor = NothingSubtle,
        ),
        contentPadding = PaddingValues(horizontal = 14.dp, vertical = 0.dp),
    ) {
        Text(
            text = text.uppercase(),
            fontFamily = FontFamily.Monospace,
            fontWeight = FontWeight.Bold,
            fontSize = 11.sp,
            letterSpacing = 0.8.sp,
            maxLines = 1,
        )
    }
}

@Composable
fun NothingStatusDot(active: Boolean, size: Dp = 8.dp) {
    Box(
        modifier = Modifier
            .size(size)
            .clip(CircleShape)
            .background(if (active) NothingRed else NothingSubtle),
    )
}

@Composable
fun NothingSectionLabel(text: String, trailing: String? = null) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = text.uppercase(),
            color = NothingMuted,
            style = MaterialTheme.typography.labelSmall,
            letterSpacing = 1.2.sp,
        )
        if (!trailing.isNullOrBlank()) {
            Text(
                text = trailing,
                color = NothingSubtle,
                style = MaterialTheme.typography.labelSmall,
            )
        }
    }
}
