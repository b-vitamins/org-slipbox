/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp

@Composable
fun SlipboxTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit,
) {
    MaterialTheme(
        colorScheme = if (darkTheme) slipboxDarkColorScheme else slipboxLightColorScheme,
        typography = slipboxTypography,
        content = content,
    )
}

internal object SlipboxDimensions {
    val touchTarget = SlipboxTokens.Geometry.TOUCH_TARGET_DP.dp
    val headerPaddingHorizontal = SlipboxTokens.Geometry.HEADER_PADDING_X_DP.dp
    val headerPaddingVertical = SlipboxTokens.Geometry.HEADER_PADDING_Y_DP.dp
    val headerMinHeight = SlipboxTokens.Geometry.HEADER_MIN_HEIGHT_DP.dp
    val readingMeasure = SlipboxTokens.Geometry.READING_MEASURE_DP.dp
    val readingPadding = SlipboxTokens.Geometry.READING_PADDING_DP.dp
    val hairline = SlipboxTokens.Geometry.HAIRLINE_DP.dp
}

private val slipboxLightColorScheme =
    lightColorScheme(
        background = Color(SlipboxTokens.Palette.PAPER_LIGHT),
        onBackground = Color(SlipboxTokens.Palette.INK_LIGHT),
        surface = Color(SlipboxTokens.Palette.SURFACE_LIGHT),
        onSurface = Color(SlipboxTokens.Palette.INK_LIGHT),
        surfaceVariant = Color(SlipboxTokens.Palette.SURFACE_LIGHT),
        onSurfaceVariant = Color(SlipboxTokens.Palette.MUTED_LIGHT),
        primary = Color(SlipboxTokens.Palette.LINK_LIGHT),
        onPrimary = Color(SlipboxTokens.Palette.PAPER_LIGHT),
        outline = Color(SlipboxTokens.Palette.HAIRLINE_LIGHT),
        outlineVariant = Color(SlipboxTokens.Palette.MUTED_3_LIGHT),
        error = Color(SlipboxTokens.Palette.MATH_ERROR_LIGHT),
        onError = Color(SlipboxTokens.Palette.PAPER_LIGHT),
    )

private val slipboxDarkColorScheme =
    darkColorScheme(
        background = Color(SlipboxTokens.Palette.PAPER_DARK),
        onBackground = Color(SlipboxTokens.Palette.INK_DARK),
        surface = Color(SlipboxTokens.Palette.SURFACE_DARK),
        onSurface = Color(SlipboxTokens.Palette.INK_DARK),
        surfaceVariant = Color(SlipboxTokens.Palette.SURFACE_DARK),
        onSurfaceVariant = Color(SlipboxTokens.Palette.MUTED_DARK),
        primary = Color(SlipboxTokens.Palette.LINK_DARK),
        onPrimary = Color(SlipboxTokens.Palette.PAPER_DARK),
        outline = Color(SlipboxTokens.Palette.HAIRLINE_DARK),
        outlineVariant = Color(SlipboxTokens.Palette.MUTED_3_DARK),
        error = Color(SlipboxTokens.Palette.MATH_ERROR_DARK),
        onError = Color(SlipboxTokens.Palette.PAPER_DARK),
    )

private val slipboxTypography =
    Typography(
        headlineLarge =
            TextStyle(
                fontFamily = FontFamily.Default,
                fontSize = SlipboxTokens.Type.H1_SIZE_SP.sp,
                lineHeight = SlipboxTokens.Type.H1_LINE_SP.sp,
                letterSpacing = SlipboxTokens.Type.LETTER_SPACING_EM.em,
            ),
        headlineMedium =
            TextStyle(
                fontFamily = FontFamily.Default,
                fontSize = SlipboxTokens.Type.H2_SIZE_SP.sp,
                lineHeight = SlipboxTokens.Type.H2_LINE_SP.sp,
                letterSpacing = SlipboxTokens.Type.LETTER_SPACING_EM.em,
            ),
        bodyLarge =
            TextStyle(
                fontFamily = FontFamily.Default,
                fontSize = SlipboxTokens.Type.BODY_SIZE_SP.sp,
                lineHeight = SlipboxTokens.Type.BODY_LINE_SP.sp,
                letterSpacing = SlipboxTokens.Type.LETTER_SPACING_EM.em,
            ),
        labelLarge =
            TextStyle(
                fontFamily = FontFamily.Default,
                fontSize = SlipboxTokens.Type.BODY_SIZE_SP.sp,
                lineHeight = SlipboxTokens.Type.BODY_LINE_SP.sp,
                letterSpacing = SlipboxTokens.Type.LETTER_SPACING_EM.em,
            ),
    )
