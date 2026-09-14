/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import android.os.Build
import androidx.activity.ComponentActivity
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.input.InputMode
import androidx.compose.ui.input.InputModeManager
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalInputModeManager
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertContentDescriptionEquals
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.requestFocus
import androidx.compose.ui.unit.Density
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.SdkSuppress
import io.github.b_vitamins.slipbox.ui.settings.ReadingSettings
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTokens
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/** What the surface measures, in the pixels the device actually paints it with. */
@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
@SdkSuppress(minSdkVersion = Build.VERSION_CODES.O)
class ProportionTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    private lateinit var density: Density
    private lateinit var inputMode: InputModeManager
    private var shown by mutableStateOf(VISUAL_CASES.first())

    private fun show() {
        composeRule.setContent {
            shown.Content {
                density = LocalDensity.current
                inputMode = LocalInputModeManager.current
                SpecimenNote(settings = remember { ReadingSettings(MemoryStore()) })
            }
        }
    }

    private fun show(viewport: VisualCase) {
        composeRule.runOnIdle { shown = viewport }
        composeRule.waitForIdle()
    }

    @Test
    fun aMarkKeepsItsOwnBoundsInsideTheTargetThatReachesIt() {
        show()
        val control = composeRule.onNodeWithContentDescription(BACK, useUnmergedTree = true)
        val size = control.fetchSemanticsNode().size
        val target = SlipboxDimensions.touchTarget.pixels(density)
        assertEquals("target width", target, size.width.toFloat(), 1f)
        assertEquals("target height", target, size.height.toFloat(), 1f)

        val pixels = control.captureToImage().pixels()
        val mark = pixels.markBounds(pixels[0, 0])
        val frame = SlipboxDimensions.glyph.pixels(density)
        assertTrue("mark width ${mark.width}px of ${frame}px", mark.width <= frame + 1)
        assertTrue("mark height ${mark.height}px of ${frame}px", mark.height <= frame + 1)
        assertTrue("the mark is drawn", mark.width >= frame / 2 && mark.height >= frame / 2)

        Evidence.record(
            "proportion-mark",
            Record()
                .size("density", density.density)
                .count("targetWidthPx", size.width)
                .count("targetHeightPx", size.height)
                .count("markWidthPx", mark.width)
                .count("markHeightPx", mark.height)
                .size("frameDp", SlipboxTokens.Geometry.GLYPH_DP)
                .size("targetDp", SlipboxTokens.Geometry.TOUCH_TARGET_DP),
        )
    }

    @Test
    fun theBackAffordanceIsWrittenOnce() {
        show()
        assertEquals(
            1,
            composeRule
                .onAllNodesWithContentDescription(BACK, useUnmergedTree = true)
                .fetchSemanticsNodes()
                .size,
        )
        composeRule.onNodeWithText(BACK).assertDoesNotExist()
    }

    @Test
    fun theTitleSitsBetweenTheMarkAndTheTargetAroundIt() {
        show()
        val title = composeRule.onNodeWithText(Specimen.TITLE).fetchSemanticsNode().size.height
        assertTrue("title ${title}px", title > SlipboxDimensions.glyph.pixels(density))
        assertTrue("title ${title}px", title < SlipboxDimensions.touchTarget.pixels(density))
    }

    @Test
    fun theHeaderControlsClearTheTitleAtEitherMeasure() {
        show()
        for (viewport in MEASURES) {
            show(viewport)
            val back = composeRule.onNodeWithContentDescription(BACK).bounds()
            val title = composeRule.onNodeWithText(Specimen.TITLE).bounds()
            val appearance = composeRule.onNodeWithText(APPEARANCE).bounds()
            val target = SlipboxDimensions.touchTarget.pixels(density)
            assertTrue("${viewport.label}: the mark clears the title", back.right <= title.left)
            assertTrue(
                "${viewport.label}: the title clears the control",
                title.right <= appearance.left,
            )
            assertEquals("${viewport.label}: back target", target, back.height, 1f)
            assertTrue("${viewport.label}: back target", back.width >= target - 1f)
            assertTrue("${viewport.label}: appearance target", appearance.width >= target - 1f)
            assertEquals(
                "${viewport.label}: the title shares the controls' line",
                back.middle,
                title.middle,
                1f,
            )
            assertEquals(
                "${viewport.label}: both controls share one line",
                back.middle,
                appearance.middle,
                1f,
            )
        }
    }

    @Test
    fun theColumnKeepsItsMeasureAndNothingReachesPastIt() {
        show()
        for (viewport in MEASURES) {
            show(viewport)
            val surface = viewport.width.pixels(density)
            val measured =
                minOf(viewport.width.value, SlipboxTokens.Geometry.READING_MEASURE_DP) -
                    2 * SlipboxTokens.Geometry.READING_PADDING_DP
            val column = composeRule.onNodeWithTag(Specimen.COLUMN).bounds()
            assertEquals(
                "${viewport.label}: the column measure",
                measured * density.density,
                column.width,
                2f,
            )
            val block = composeRule.onNodeWithTag(Specimen.BLOCK).bounds()
            assertTrue(
                "${viewport.label}: the block keeps the measure",
                block.width <= column.width,
            )
            for ((name, node) in listOf("column" to column, "block" to block)) {
                assertTrue("${viewport.label}: $name starts on the surface", node.left >= 0f)
                assertTrue(
                    "${viewport.label}: $name ends on the surface",
                    node.right <= surface + 1f,
                )
            }
        }
    }

    @Test
    fun everyControlKeepsItsRoleNameAndFocus() {
        show()
        composeRule
            .onNodeWithContentDescription(BACK)
            .assertContentDescriptionEquals(BACK)
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Button))
            .assertIsEnabled()
            .takesFocus()
        composeRule
            .onNodeWithText(APPEARANCE)
            .assertTextEquals(APPEARANCE)
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Button))
            .assertIsEnabled()
            .takesFocus()
    }

    /** A control takes focus only once the window holds it and the input is not touch. */
    private fun SemanticsNodeInteraction.takesFocus() {
        composeRule.waitUntil(FOCUS_TIMEOUT) { composeRule.activity.hasWindowFocus() }
        assertTrue(
            "the surface leaves touch input",
            composeRule.runOnUiThread { inputMode.requestInputMode(InputMode.Keyboard) },
        )
        requestFocus()
        assertIsFocused()
    }

    private companion object {
        const val BACK = "Back"
        const val APPEARANCE = "Appearance"
        const val FOCUS_TIMEOUT = 5_000L

        val MEASURES = VISUAL_CASES.filter { !it.dark && it.fontScale == 1f }
    }
}
