/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.document

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.sp
import io.github.b_vitamins.slipbox.ui.theme.SlipboxAppearance
import io.github.b_vitamins.slipbox.ui.theme.SlipboxMotion
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTokens
import io.github.b_vitamins.slipbox.ui.theme.paintsDark
import io.github.b_vitamins.slipbox.ui.theme.rememberPlatformMotionScale

/** Resolve platform font scaling before converting physical pixels to CSS pixels. */
internal fun Density.cssPx(size: TextUnit): Float = size.toPx() / density

/**
 * Preserve the line-to-font ratio; nonlinear scaling of the line length changes it.
 */
internal fun Density.cssLine(size: TextUnit, line: TextUnit): Float =
    cssPx(size) * (line.value / size.value)

internal fun documentPresentation(
    density: Density,
    dark: Boolean,
    motion: SlipboxMotion,
    availableWidth: Dp,
): DocumentPresentation {
    val type =
        with(density) {
            DocumentTypography(
                bodySize = cssPx(SlipboxTokens.Type.BODY_SIZE_SP.sp),
                bodyLine =
                    cssLine(
                        SlipboxTokens.Type.BODY_SIZE_SP.sp,
                        SlipboxTokens.Type.BODY_LINE_SP.sp,
                    ),
                h1Size = cssPx(SlipboxTokens.Type.H1_SIZE_SP.sp),
                h1Line =
                    cssLine(SlipboxTokens.Type.H1_SIZE_SP.sp, SlipboxTokens.Type.H1_LINE_SP.sp),
                h2Size = cssPx(SlipboxTokens.Type.H2_SIZE_SP.sp),
                h2Line =
                    cssLine(SlipboxTokens.Type.H2_SIZE_SP.sp, SlipboxTokens.Type.H2_LINE_SP.sp),
                codeSize = cssPx(SlipboxTokens.Type.CODE_SIZE_SP.sp),
                codeLine =
                    cssLine(
                        SlipboxTokens.Type.CODE_SIZE_SP.sp,
                        SlipboxTokens.Type.CODE_LINE_SP.sp,
                    ),
                chromeSize = cssPx(SlipboxTokens.Type.CHROME_SIZE_SP.sp),
                tableSize = cssPx(SlipboxTokens.Type.TABLE_SIZE_SP.sp),
                letterSpacingEm = SlipboxTokens.Type.LETTER_SPACING_EM,
            )
        }
    return DocumentPresentation(
        theme = if (dark) DocumentTheme.Dark else DocumentTheme.Light,
        typography = type,
        geometry =
            DocumentGeometry(
                columnWidth = SlipboxTokens.Geometry.READING_MEASURE_DP,
                padding = SlipboxTokens.Geometry.READING_PADDING_DP,
                touchTarget = SlipboxTokens.Geometry.TOUCH_TARGET_DP,
                availableWidth = availableWidth.value,
            ),
        motion =
            DocumentMotion(
                columnMs = motion.document(SlipboxTokens.Motion.COLUMN_MS),
                shadowMs = motion.document(SlipboxTokens.Motion.SHADOW_MS),
                opacityMs = motion.document(SlipboxTokens.Motion.OPACITY_MS),
                crossfadeMs = motion.document(SlipboxTokens.Motion.CROSSFADE_MS),
                reduced = motion.reduced,
            ),
    )
}

@Composable
internal fun rememberDocumentPresentation(
    appearance: SlipboxAppearance,
    reduceMotion: Boolean,
    availableWidth: Dp,
): DocumentPresentation {
    val density = LocalDensity.current
    val dark = appearance.paintsDark(isSystemInDarkTheme())
    val motion = SlipboxMotion(rememberPlatformMotionScale(), reduceMotion)
    return remember(density, dark, motion, availableWidth) {
        documentPresentation(density, dark, motion, availableWidth)
    }
}
