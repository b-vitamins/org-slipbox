/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.requiredWidth
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.assertIsEqualTo
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.test.ext.junit.runners.AndroidJUnit4
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class ReadingColumnTest {

    @get:Rule val composeRule = createComposeRule()

    @Test
    fun aNarrowSurfaceGivesTheColumnEverythingButItsPadding() {
        setColumnIn(NARROW)

        assertColumn(width = NARROW - PADDING * 2, inset = PADDING)
    }

    @Test
    fun aWideSurfaceCapsTheColumnAtTheMeasureAndCentresIt() {
        setColumnIn(WIDE)

        assertColumn(width = MEASURE - PADDING * 2, inset = (WIDE - MEASURE) / 2 + PADDING)
    }

    private fun setColumnIn(width: Dp) {
        composeRule.setContent {
            Box(modifier = Modifier.requiredWidth(width).testTag(SURFACE)) {
                ReadingColumn {
                    Box(modifier = Modifier.fillMaxWidth().height(24.dp).testTag(BODY))
                }
            }
        }
    }

    private fun assertColumn(width: Dp, inset: Dp) {
        // Compare to the injected surface: its wide layout may overflow the device root.
        val surface = composeRule.onNodeWithTag(SURFACE).getUnclippedBoundsInRoot()
        val body = composeRule.onNodeWithTag(BODY).getUnclippedBoundsInRoot()

        (body.right - body.left).assertIsEqualTo(width, "the column's width")
        (body.left - surface.left).assertIsEqualTo(inset, "the column's inset from the surface")
    }

    private companion object {
        val NARROW = 320.dp
        val WIDE = 900.dp
        val MEASURE = SlipboxDimensions.readingMeasure
        val PADDING = SlipboxDimensions.readingPadding
        const val SURFACE = "reading-column-surface"
        const val BODY = "reading-column-body"
    }
}
