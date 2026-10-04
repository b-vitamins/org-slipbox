/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.theme

import android.content.Context
import android.content.ContextWrapper
import android.graphics.Color as AndroidColor
import android.os.Build
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.background
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.tappableElement
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.LineHeightStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import androidx.core.view.WindowCompat

/** Which palette scheme the reader paints in; `System` follows the platform. */
enum class SlipboxAppearance {
    System,
    Light,
    Dark,
}

@Composable
fun SlipboxTheme(
    appearance: SlipboxAppearance = SlipboxAppearance.System,
    content: @Composable () -> Unit,
) {
    val systemInDark = isSystemInDarkTheme()
    val dark = appearance.paintsDark(systemInDark)
    val colors = if (dark) slipboxDarkColors else slipboxLightColors
    val view = LocalView.current
    SideEffect {
        val activity = view.context.componentActivity() ?: return@SideEffect
        activity.enableEdgeToEdge(
            statusBarStyle = systemBarStyle(dark, AndroidColor.TRANSPARENT),
            navigationBarStyle =
                systemBarStyle(
                    dark,
                    if (dark) {
                        SlipboxTokens.Palette.PAPER_DARK.toInt()
                    } else {
                        SlipboxTokens.Palette.PAPER_LIGHT.toInt()
                    },
                ),
        )
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            activity.window.isNavigationBarContrastEnforced = false
        }
        WindowCompat.getInsetsController(activity.window, view).apply {
            isAppearanceLightStatusBars = !dark
            isAppearanceLightNavigationBars = !dark
        }
    }
    CompositionLocalProvider(LocalSlipboxColors provides colors) {
        MaterialTheme(
            colorScheme = if (dark) slipboxDarkColorScheme else slipboxLightColorScheme,
            typography = slipboxTypography,
        ) {
            Box(Modifier.fillMaxSize()) {
                content()
                NavigationBarProtection(Modifier.align(Alignment.BottomCenter))
            }
        }
    }
}

/** Opaque only for button navigation; gesture navigation remains edge-to-edge. */
@Composable
private fun NavigationBarProtection(modifier: Modifier = Modifier) {
    val density = LocalDensity.current
    val bottom = WindowInsets.tappableElement.getBottom(density)
    if (bottom != 0) {
        Box(
            modifier =
                modifier
                    .fillMaxWidth()
                    .height(with(density) { bottom.toDp() })
                    .background(MaterialTheme.colorScheme.background),
        )
    }
}

private fun systemBarStyle(dark: Boolean, scrim: Int): SystemBarStyle =
    if (dark) {
        SystemBarStyle.dark(scrim)
    } else {
        SystemBarStyle.light(scrim, scrim)
    }

private tailrec fun Context.componentActivity(): ComponentActivity? =
    when (this) {
        is ComponentActivity -> this
        is ContextWrapper -> baseContext.componentActivity()
        else -> null
    }

internal fun SlipboxAppearance.paintsDark(systemInDark: Boolean): Boolean =
    when (this) {
        SlipboxAppearance.System -> systemInDark
        SlipboxAppearance.Light -> false
        SlipboxAppearance.Dark -> true
    }

/** Reading palette, including tones not represented by Material color roles. */
@Immutable
internal data class SlipboxColors(
    val paper: Color,
    val surface: Color,
    val ink: Color,
    val muted: Color,
    val mutedSecondary: Color,
    val rule: Color,
    val hairline: Color,
    val link: Color,
    val linkVisited: Color,
    val mathError: Color,
    val scrim: Color,
)

internal object SlipboxTheme {
    val colors: SlipboxColors
        @Composable @ReadOnlyComposable get() = LocalSlipboxColors.current
}

internal object SlipboxDimensions {
    val touchTarget = SlipboxTokens.Geometry.TOUCH_TARGET_DP.dp
    val glyph = SlipboxTokens.Geometry.GLYPH_DP.dp
    val headerPaddingHorizontal = SlipboxTokens.Geometry.HEADER_PADDING_X_DP.dp
    val headerPaddingVertical = SlipboxTokens.Geometry.HEADER_PADDING_Y_DP.dp
    val headerMinHeight = SlipboxTokens.Geometry.HEADER_MIN_HEIGHT_DP.dp
    val readingMeasure = SlipboxTokens.Geometry.READING_MEASURE_DP.dp
    val readingPadding = SlipboxTokens.Geometry.READING_PADDING_DP.dp
    val hairline = SlipboxTokens.Geometry.HAIRLINE_DP.dp
    val blockCorner = SlipboxTokens.Geometry.BLOCK_CORNER_DP.dp
}

private val LocalSlipboxColors =
    staticCompositionLocalOf<SlipboxColors> { error("no SlipboxTheme encloses this content") }

private val slipboxLightColors =
    SlipboxColors(
        paper = Color(SlipboxTokens.Palette.PAPER_LIGHT),
        surface = Color(SlipboxTokens.Palette.SURFACE_LIGHT),
        ink = Color(SlipboxTokens.Palette.INK_LIGHT),
        muted = Color(SlipboxTokens.Palette.MUTED_LIGHT),
        mutedSecondary = Color(SlipboxTokens.Palette.MUTED_2_LIGHT),
        rule = Color(SlipboxTokens.Palette.MUTED_3_LIGHT),
        hairline = Color(SlipboxTokens.Palette.HAIRLINE_LIGHT),
        link = Color(SlipboxTokens.Palette.LINK_LIGHT),
        linkVisited = Color(SlipboxTokens.Palette.LINK_VISITED_LIGHT),
        mathError = Color(SlipboxTokens.Palette.MATH_ERROR_LIGHT),
        scrim = Color(SlipboxTokens.Palette.SCRIM_LIGHT),
    )

private val slipboxDarkColors =
    SlipboxColors(
        paper = Color(SlipboxTokens.Palette.PAPER_DARK),
        surface = Color(SlipboxTokens.Palette.SURFACE_DARK),
        ink = Color(SlipboxTokens.Palette.INK_DARK),
        muted = Color(SlipboxTokens.Palette.MUTED_DARK),
        mutedSecondary = Color(SlipboxTokens.Palette.MUTED_2_DARK),
        rule = Color(SlipboxTokens.Palette.MUTED_3_DARK),
        hairline = Color(SlipboxTokens.Palette.HAIRLINE_DARK),
        link = Color(SlipboxTokens.Palette.LINK_DARK),
        linkVisited = Color(SlipboxTokens.Palette.LINK_VISITED_DARK),
        mathError = Color(SlipboxTokens.Palette.MATH_ERROR_DARK),
        scrim = Color(SlipboxTokens.Palette.SCRIM_DARK),
    )

private val slipboxLightColorScheme =
    lightColorScheme(
        background = Color(SlipboxTokens.Palette.PAPER_LIGHT),
        onBackground = Color(SlipboxTokens.Palette.INK_LIGHT),
        surface = Color(SlipboxTokens.Palette.SURFACE_LIGHT),
        onSurface = Color(SlipboxTokens.Palette.INK_LIGHT),
        surfaceVariant = Color(SlipboxTokens.Palette.SURFACE_LIGHT),
        onSurfaceVariant = Color(SlipboxTokens.Palette.MUTED_LIGHT),
        surfaceTint = Color.Transparent,
        surfaceDim = Color(SlipboxTokens.Palette.SURFACE_DIM_LIGHT),
        surfaceBright = Color(SlipboxTokens.Palette.SURFACE_LIGHT),
        surfaceContainerLowest = Color(SlipboxTokens.Palette.SURFACE_LIGHT),
        surfaceContainerLow = Color(SlipboxTokens.Palette.PAPER_LIGHT),
        surfaceContainer = Color(SlipboxTokens.Palette.SURFACE_CONTAINER_LIGHT),
        surfaceContainerHigh = Color(SlipboxTokens.Palette.SURFACE_DIM_LIGHT),
        surfaceContainerHighest = Color(SlipboxTokens.Palette.MUTED_3_LIGHT),
        primary = Color(SlipboxTokens.Palette.LINK_LIGHT),
        onPrimary = Color(SlipboxTokens.Palette.PAPER_LIGHT),
        primaryContainer = Color(SlipboxTokens.Palette.MUTED_3_LIGHT),
        onPrimaryContainer = Color(SlipboxTokens.Palette.INK_LIGHT),
        inversePrimary = Color(SlipboxTokens.Palette.LINK_DARK),
        secondary = Color(SlipboxTokens.Palette.MUTED_LIGHT),
        onSecondary = Color(SlipboxTokens.Palette.SURFACE_LIGHT),
        secondaryContainer = Color(SlipboxTokens.Palette.MUTED_3_LIGHT),
        onSecondaryContainer = Color(SlipboxTokens.Palette.INK_LIGHT),
        tertiary = Color(SlipboxTokens.Palette.LINK_VISITED_LIGHT),
        onTertiary = Color(SlipboxTokens.Palette.SURFACE_LIGHT),
        tertiaryContainer = Color(SlipboxTokens.Palette.MUTED_3_LIGHT),
        onTertiaryContainer = Color(SlipboxTokens.Palette.INK_LIGHT),
        outline = Color(SlipboxTokens.Palette.HAIRLINE_LIGHT),
        outlineVariant = Color(SlipboxTokens.Palette.MUTED_3_LIGHT),
        scrim = Color(SlipboxTokens.Palette.SCRIM_LIGHT),
        inverseSurface = Color(SlipboxTokens.Palette.INK_LIGHT),
        inverseOnSurface = Color(SlipboxTokens.Palette.SURFACE_LIGHT),
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
        surfaceTint = Color.Transparent,
        surfaceDim = Color(SlipboxTokens.Palette.SURFACE_DIM_DARK),
        surfaceBright = Color(SlipboxTokens.Palette.SURFACE_BRIGHT_DARK),
        surfaceContainerLowest = Color(SlipboxTokens.Palette.SURFACE_DIM_DARK),
        surfaceContainerLow = Color(SlipboxTokens.Palette.PAPER_DARK),
        surfaceContainer = Color(SlipboxTokens.Palette.SURFACE_CONTAINER_DARK),
        surfaceContainerHigh = Color(SlipboxTokens.Palette.SURFACE_DARK),
        surfaceContainerHighest = Color(SlipboxTokens.Palette.MUTED_3_DARK),
        primary = Color(SlipboxTokens.Palette.LINK_DARK),
        onPrimary = Color(SlipboxTokens.Palette.PAPER_DARK),
        primaryContainer = Color(SlipboxTokens.Palette.MUTED_3_DARK),
        onPrimaryContainer = Color(SlipboxTokens.Palette.INK_DARK),
        inversePrimary = Color(SlipboxTokens.Palette.LINK_LIGHT),
        secondary = Color(SlipboxTokens.Palette.MUTED_DARK),
        onSecondary = Color(SlipboxTokens.Palette.SURFACE_DARK),
        secondaryContainer = Color(SlipboxTokens.Palette.MUTED_3_DARK),
        onSecondaryContainer = Color(SlipboxTokens.Palette.INK_DARK),
        tertiary = Color(SlipboxTokens.Palette.LINK_VISITED_DARK),
        onTertiary = Color(SlipboxTokens.Palette.SURFACE_DARK),
        tertiaryContainer = Color(SlipboxTokens.Palette.MUTED_3_DARK),
        onTertiaryContainer = Color(SlipboxTokens.Palette.INK_DARK),
        outline = Color(SlipboxTokens.Palette.HAIRLINE_DARK),
        outlineVariant = Color(SlipboxTokens.Palette.MUTED_3_DARK),
        scrim = Color(SlipboxTokens.Palette.SCRIM_DARK),
        inverseSurface = Color(SlipboxTokens.Palette.INK_DARK),
        inverseOnSurface = Color(SlipboxTokens.Palette.SURFACE_DARK),
        error = Color(SlipboxTokens.Palette.MATH_ERROR_DARK),
        onError = Color(SlipboxTokens.Palette.PAPER_DARK),
    )

/** Keep full leading at paragraph edges to match document line metrics. */
private val slipboxLineHeight =
    LineHeightStyle(
        alignment = LineHeightStyle.Alignment.Center,
        trim = LineHeightStyle.Trim.None,
    )

private val slipboxTypography =
    Typography(
        headlineLarge =
            TextStyle(
                fontFamily = FontFamily.Default,
                fontWeight = FontWeight.Medium,
                fontSize = SlipboxTokens.Type.H1_SIZE_SP.sp,
                lineHeight = SlipboxTokens.Type.H1_LINE_SP.sp,
                lineHeightStyle = slipboxLineHeight,
                letterSpacing = SlipboxTokens.Type.LETTER_SPACING_EM.em,
            ),
        headlineMedium =
            TextStyle(
                fontFamily = FontFamily.Default,
                fontWeight = FontWeight.Medium,
                fontSize = SlipboxTokens.Type.H2_SIZE_SP.sp,
                lineHeight = SlipboxTokens.Type.H2_LINE_SP.sp,
                lineHeightStyle = slipboxLineHeight,
                letterSpacing = SlipboxTokens.Type.LETTER_SPACING_EM.em,
            ),
        titleLarge =
            TextStyle(
                fontFamily = FontFamily.Default,
                fontWeight = FontWeight.Medium,
                fontSize = SlipboxTokens.Type.TITLE_SIZE_SP.sp,
                lineHeight = SlipboxTokens.Type.TITLE_LINE_SP.sp,
                lineHeightStyle = slipboxLineHeight,
                letterSpacing = SlipboxTokens.Type.LETTER_SPACING_EM.em,
            ),
        titleMedium =
            TextStyle(
                fontFamily = FontFamily.Default,
                fontWeight = FontWeight.Medium,
                fontSize = SlipboxTokens.Type.SUBHEAD_SIZE_SP.sp,
                lineHeight = SlipboxTokens.Type.SUBHEAD_LINE_SP.sp,
                lineHeightStyle = slipboxLineHeight,
                letterSpacing = SlipboxTokens.Type.LETTER_SPACING_EM.em,
            ),
        bodyLarge =
            TextStyle(
                fontFamily = FontFamily.Default,
                fontSize = SlipboxTokens.Type.BODY_SIZE_SP.sp,
                lineHeight = SlipboxTokens.Type.BODY_LINE_SP.sp,
                lineHeightStyle = slipboxLineHeight,
                letterSpacing = SlipboxTokens.Type.LETTER_SPACING_EM.em,
            ),
        bodyMedium =
            TextStyle(
                fontFamily = FontFamily.Default,
                fontSize = SlipboxTokens.Type.SECONDARY_SIZE_SP.sp,
                lineHeight = SlipboxTokens.Type.SECONDARY_LINE_SP.sp,
                lineHeightStyle = slipboxLineHeight,
            ),
        bodySmall =
            TextStyle(
                fontFamily = FontFamily.Default,
                fontSize = SlipboxTokens.Type.CAPTION_SIZE_SP.sp,
                lineHeight = SlipboxTokens.Type.CAPTION_LINE_SP.sp,
                lineHeightStyle = slipboxLineHeight,
            ),
        labelLarge =
            TextStyle(
                fontFamily = FontFamily.Default,
                fontSize = SlipboxTokens.Type.BODY_SIZE_SP.sp,
                lineHeight = SlipboxTokens.Type.BODY_LINE_SP.sp,
                lineHeightStyle = slipboxLineHeight,
                letterSpacing = SlipboxTokens.Type.LETTER_SPACING_EM.em,
            ),
        labelMedium =
            TextStyle(
                fontFamily = FontFamily.Default,
                fontWeight = FontWeight.Medium,
                fontSize = SlipboxTokens.Type.CAPTION_SIZE_SP.sp,
                lineHeight = SlipboxTokens.Type.CAPTION_LINE_SP.sp,
                lineHeightStyle = slipboxLineHeight,
            ),
        labelSmall =
            TextStyle(
                fontFamily = FontFamily.Default,
                fontSize = SlipboxTokens.Type.CHROME_SIZE_SP.sp,
                lineHeight = SlipboxTokens.Type.CODE_LINE_SP.sp,
                lineHeightStyle = slipboxLineHeight,
            ),
    )
