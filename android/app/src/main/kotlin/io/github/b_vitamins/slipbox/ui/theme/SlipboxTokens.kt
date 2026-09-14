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

        const val MUTED_2_LIGHT: Long = 0xFF717171
        const val MUTED_2_DARK: Long = 0xFF878789

        const val MUTED_3_LIGHT: Long = 0xFFDDDDDD
        const val MUTED_3_DARK: Long = 0xFF3E3E40

        const val HAIRLINE_LIGHT: Long = 0xFFDADADA
        const val HAIRLINE_DARK: Long = 0xFF454547

        const val LINK_LIGHT: Long = 0xFF0A6ED1
        const val LINK_DARK: Long = 0xFF5AA6EA

        const val LINK_VISITED_LIGHT: Long = 0xFF9333C4
        const val LINK_VISITED_DARK: Long = 0xFFB78ADA

        const val MATH_ERROR_LIGHT: Long = 0xFFCC0000
        const val MATH_ERROR_DARK: Long = 0xFFFF6B6B

        /** The web surface's `--edge-shadow`, carried as the wash under a reveal. */
        const val SCRIM_LIGHT: Long = 0x1F000000
        const val SCRIM_DARK: Long = 0x80000000
    }

    object Type {
        const val BODY_SIZE_SP: Float = 17f
        const val BODY_LINE_SP: Float = 24f
        const val LETTER_SPACING_EM: Float = -0.0024f

        const val H1_SIZE_SP: Float = 28f
        const val H1_LINE_SP: Float = 34f

        const val H2_SIZE_SP: Float = 25f
        const val H2_LINE_SP: Float = 31f

        const val CODE_SIZE_SP: Float = 13f
        const val CODE_LINE_SP: Float = 17f

        /** The rendered document's own fixed sizes: source-block chrome and tables. */
        const val CHROME_SIZE_SP: Float = 12f
        const val TABLE_SIZE_SP: Float = 15f
    }

    object Geometry {
        const val TOUCH_TARGET_DP: Float = 48f
        const val HEADER_PADDING_Y_DP: Float = 6f
        const val HEADER_PADDING_X_DP: Float = 16f
        const val HEADER_MIN_HEIGHT_DP: Float = TOUCH_TARGET_DP + 2 * HEADER_PADDING_Y_DP

        const val READING_MEASURE_DP: Float = 625f

        const val READING_PADDING_DP: Float = 16f
        const val HAIRLINE_DP: Float = 1f

        /** A glyph's own frame, sized apart from the target that reaches it. */
        const val GLYPH_DP: Float = 20f

        /** The rendered document's own source-block corner. */
        const val BLOCK_CORNER_DP: Float = 8f
    }

    object Motion {
        const val COLUMN_MS: Int = 200
        const val SHADOW_MS: Int = 100
        const val OPACITY_MS: Int = 75
        const val CROSSFADE_MS: Int = 150

        /** The control points of the web surface's `--ease-settle`. */
        const val SETTLE_X1: Float = 0.19f
        const val SETTLE_Y1: Float = 1f
        const val SETTLE_X2: Float = 0.22f
        const val SETTLE_Y2: Float = 1f
    }
}
