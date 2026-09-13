/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.theme

/** Web reading tokens, using coarse-pointer geometry and a native 48dp touch floor. */
internal object SlipboxTokens {

    object Palette {
        const val PAPER_LIGHT: Long = 0xFFFAFAFC
        const val PAPER_DARK: Long = 0xFF16161A

        const val SURFACE_LIGHT: Long = 0xFFFFFFFF
        const val SURFACE_DARK: Long = 0xFF1D1D21

        const val INK_LIGHT: Long = 0xFF333333
        const val INK_DARK: Long = 0xFFD7D7DA

        const val MUTED_LIGHT: Long = 0xFF6B6B6B
        const val MUTED_DARK: Long = 0xFF8D8D90

        const val MUTED_3_LIGHT: Long = 0xFFDDDDDD
        const val MUTED_3_DARK: Long = 0xFF3E3E40

        const val HAIRLINE_LIGHT: Long = 0xFFDADADA
        const val HAIRLINE_DARK: Long = 0xFF454547

        const val LINK_LIGHT: Long = 0xFF0A6ED1
        const val LINK_DARK: Long = 0xFF5AA6EA

        const val MATH_ERROR_LIGHT: Long = 0xFFCC0000
        const val MATH_ERROR_DARK: Long = 0xFFFF6B6B
    }

    object Type {
        const val BODY_SIZE_SP: Float = 17f
        const val BODY_LINE_SP: Float = 24f
        const val LETTER_SPACING_EM: Float = -0.0024f

        const val H1_SIZE_SP: Float = 28f
        const val H1_LINE_SP: Float = 34f

        const val H2_SIZE_SP: Float = 25f
        const val H2_LINE_SP: Float = 31f
    }

    object Geometry {
        const val TOUCH_TARGET_DP: Float = 48f
        const val HEADER_PADDING_Y_DP: Float = 6f
        const val HEADER_PADDING_X_DP: Float = 16f
        const val HEADER_MIN_HEIGHT_DP: Float = TOUCH_TARGET_DP + 2 * HEADER_PADDING_Y_DP

        const val READING_MEASURE_DP: Float = 625f

        const val READING_PADDING_DP: Float = 16f
        const val HAIRLINE_DP: Float = 1f
    }
}
