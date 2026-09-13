/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.theme

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class SlipboxTokensTest {

    private val pairs =
        mapOf(
            "paper" to (SlipboxTokens.Palette.PAPER_LIGHT to SlipboxTokens.Palette.PAPER_DARK),
            "surface" to
                (SlipboxTokens.Palette.SURFACE_LIGHT to SlipboxTokens.Palette.SURFACE_DARK),
            "ink" to (SlipboxTokens.Palette.INK_LIGHT to SlipboxTokens.Palette.INK_DARK),
            "muted" to (SlipboxTokens.Palette.MUTED_LIGHT to SlipboxTokens.Palette.MUTED_DARK),
            "muted-3" to
                (SlipboxTokens.Palette.MUTED_3_LIGHT to SlipboxTokens.Palette.MUTED_3_DARK),
            "hairline" to
                (SlipboxTokens.Palette.HAIRLINE_LIGHT to SlipboxTokens.Palette.HAIRLINE_DARK),
            "link" to (SlipboxTokens.Palette.LINK_LIGHT to SlipboxTokens.Palette.LINK_DARK),
            "math-error" to
                (SlipboxTokens.Palette.MATH_ERROR_LIGHT to SlipboxTokens.Palette.MATH_ERROR_DARK),
        )

    @Test
    fun everyToneIsFullyOpaque() {
        for ((name, pair) in pairs) {
            assertEquals("$name light alpha", 0xFFL, pair.first shr 24 and 0xFF)
            assertEquals("$name dark alpha", 0xFFL, pair.second shr 24 and 0xFF)
        }
    }

    @Test
    fun everyToneIsAPairOfTwoDifferentTones() {
        for ((name, pair) in pairs) {
            assertNotEquals(name, pair.first, pair.second)
        }
    }

    @Test
    fun aLineLeavesRoomForItsSize() {
        assertTrue(SlipboxTokens.Type.BODY_LINE_SP > SlipboxTokens.Type.BODY_SIZE_SP)
        assertTrue(SlipboxTokens.Type.H1_LINE_SP > SlipboxTokens.Type.H1_SIZE_SP)
        assertTrue(SlipboxTokens.Type.H2_LINE_SP > SlipboxTokens.Type.H2_SIZE_SP)
    }

    @Test
    fun theScaleDescendsFromHeadingToProse() {
        assertTrue(SlipboxTokens.Type.H1_SIZE_SP > SlipboxTokens.Type.H2_SIZE_SP)
        assertTrue(SlipboxTokens.Type.H2_SIZE_SP > SlipboxTokens.Type.BODY_SIZE_SP)
    }

    @Test
    fun theTrackingIsATightening() {
        assertTrue(SlipboxTokens.Type.LETTER_SPACING_EM < 0f)
        assertTrue(SlipboxTokens.Type.LETTER_SPACING_EM > -0.01f)
    }

    @Test
    fun everyControlClearsThePlatformsTouchTarget() {
        assertTrue(SlipboxTokens.Geometry.TOUCH_TARGET_DP >= 48f)
    }

    @Test
    fun theHeaderIsNeverSmallerThanTheControlInIt() {
        assertEquals(
            SlipboxTokens.Geometry.TOUCH_TARGET_DP + 2 * SlipboxTokens.Geometry.HEADER_PADDING_Y_DP,
            SlipboxTokens.Geometry.HEADER_MIN_HEIGHT_DP,
            0f,
        )
    }

    @Test
    fun theMeasureIsWiderThanThePaddingItIsReadWith() {
        assertTrue(
            SlipboxTokens.Geometry.READING_MEASURE_DP >
                2 * SlipboxTokens.Geometry.READING_PADDING_DP,
        )
    }

    @Test
    fun aHairlineIsTheThinnestRuleThereIs() {
        assertEquals(1f, SlipboxTokens.Geometry.HAIRLINE_DP, 0f)
    }
}
