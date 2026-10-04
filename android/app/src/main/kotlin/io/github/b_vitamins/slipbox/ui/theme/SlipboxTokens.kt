/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.theme

/** Web reading tokens, using coarse-pointer geometry and a native 48dp touch floor. */
internal object SlipboxTokens {

    object Palette {
        const val PAPER_LIGHT: Long = 0xFFFAF9F5
        const val PAPER_DARK: Long = 0xFF262624

        const val SURFACE_LIGHT: Long = 0xFFFFFFFE
        const val SURFACE_DARK: Long = 0xFF30302E

        const val SURFACE_DIM_LIGHT: Long = 0xFFF0EEE8
        const val SURFACE_DIM_DARK: Long = 0xFF20201E
        const val SURFACE_BRIGHT_DARK: Long = 0xFF464642
        const val SURFACE_CONTAINER_LIGHT: Long = 0xFFF5F3ED
        const val SURFACE_CONTAINER_DARK: Long = 0xFF2B2B29

        const val INK_LIGHT: Long = 0xFF2D2825
        const val INK_DARK: Long = 0xFFF0EEE6

        const val MUTED_LIGHT: Long = 0xFF6F6D66
        const val MUTED_DARK: Long = 0xFFB0AEA5

        const val MUTED_2_LIGHT: Long = 0xFF77746E
        const val MUTED_2_DARK: Long = 0xFF9C9A92

        const val MUTED_3_LIGHT: Long = 0xFFE8E6DC
        const val MUTED_3_DARK: Long = 0xFF3A3936

        const val HAIRLINE_LIGHT: Long = 0xFFDEDCD1
        const val HAIRLINE_DARK: Long = 0xFF484641

        const val LINK_LIGHT: Long = 0xFFA94F35
        const val LINK_DARK: Long = 0xFFE89578

        const val LINK_VISITED_LIGHT: Long = 0xFF805841
        const val LINK_VISITED_DARK: Long = 0xFFC7A189

        const val MATH_ERROR_LIGHT: Long = 0xFFB42318
        const val MATH_ERROR_DARK: Long = 0xFFFF8A80

        /** The web surface's `--edge-shadow`, carried as the wash under a reveal. */
        const val SCRIM_LIGHT: Long = 0x1F000000
        const val SCRIM_DARK: Long = 0x990F0E0C
    }

    object Type {
        const val BODY_SIZE_SP: Float = 17f
        const val BODY_LINE_SP: Float = 25f
        const val LETTER_SPACING_EM: Float = -0.0024f

        const val H1_SIZE_SP: Float = 28f
        const val H1_LINE_SP: Float = 34f

        const val H2_SIZE_SP: Float = 23f
        const val H2_LINE_SP: Float = 29f

        const val TITLE_SIZE_SP: Float = 22f
        const val TITLE_LINE_SP: Float = 28f
        const val SUBHEAD_SIZE_SP: Float = 17f
        const val SUBHEAD_LINE_SP: Float = 23f
        const val SECONDARY_SIZE_SP: Float = 15f
        const val SECONDARY_LINE_SP: Float = 21f
        const val CAPTION_SIZE_SP: Float = 13f
        const val CAPTION_LINE_SP: Float = 18f

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
