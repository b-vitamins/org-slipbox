/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import androidx.activity.ComponentActivity
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeLeft
import androidx.compose.ui.unit.Density
import androidx.test.ext.junit.runners.AndroidJUnit4
import io.github.b_vitamins.slipbox.ui.document.DocumentPresentation
import io.github.b_vitamins.slipbox.ui.document.rememberDocumentPresentation
import io.github.b_vitamins.slipbox.ui.settings.ReadingSettings
import io.github.b_vitamins.slipbox.ui.theme.SlipboxAppearance
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTokens
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/** What the platform's own font scale does to the surface, and to what it exports. */
@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
class ScaleTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    private lateinit var density: Density
    private lateinit var exported: DocumentPresentation
    private var composed = false
    private var shown by mutableStateOf(VISUAL_CASES.first())

    private fun show(viewport: VisualCase) {
        if (composed) {
            composeRule.runOnIdle { shown = viewport }
        } else {
            shown = viewport
            composeRule.setContent {
                shown.Content {
                    density = LocalDensity.current
                    exported =
                        rememberDocumentPresentation(
                            appearance = SlipboxAppearance.System,
                            reduceMotion = false,
                            availableWidth = shown.width,
                        )
                    // Its own subtree, so a case begins where the note does rather than
                    // where the case before it was left.
                    key(shown.label) {
                        SpecimenNote(settings = remember { ReadingSettings(MemoryStore()) })
                    }
                }
            }
            composed = true
        }
        composeRule.waitForIdle()
    }

    /**
     * The device's own curve, not a multiplication: every size grows, the reading sizes
     * grow by less than the scale, and the smallest text gains the most. Lengths are
     * left where they were.
     */
    @Test
    fun theTypeGrowsWithTheReadersScaleAndByLessThanIt() {
        show(measure(320f, fontScale = 1f))
        val plain = exported.typography
        show(measure(320f, fontScale = 2f))
        val scaled = exported.typography
        val gains =
            listOf(
                "bodySize" to scaled.bodySize / plain.bodySize,
                "h1Size" to scaled.h1Size / plain.h1Size,
                "h2Size" to scaled.h2Size / plain.h2Size,
                "codeSize" to scaled.codeSize / plain.codeSize,
                "chromeSize" to scaled.chromeSize / plain.chromeSize,
                "tableSize" to scaled.tableSize / plain.tableSize,
            )
        for ((name, gain) in gains) {
            assertTrue("$name grows: $gain", gain > 1f)
            assertTrue("$name grows no more than the scale: $gain", gain <= 2f)
        }
        assertTrue("the reading size", scaled.bodySize < 2 * plain.bodySize)
        assertTrue("the heading", scaled.h1Size < 2 * plain.h1Size)
        assertTrue(
            "the smallest text gains the most",
            scaled.codeSize / plain.codeSize > scaled.h1Size / plain.h1Size,
        )
        assertEquals(
            "the measure is not a text size",
            SlipboxTokens.Geometry.READING_MEASURE_DP,
            exported.geometry.columnWidth,
            0f,
        )
        Evidence.record(
            "scale-typography",
            Record()
                .size("bodySizeAtOne", plain.bodySize)
                .size("bodySizeAtTwo", scaled.bodySize)
                .size("h1SizeAtOne", plain.h1Size)
                .size("h1SizeAtTwo", scaled.h1Size)
                .size("codeSizeAtOne", plain.codeSize)
                .size("codeSizeAtTwo", scaled.codeSize)
                .size("codeGain", scaled.codeSize / plain.codeSize)
                .size("h1Gain", scaled.h1Size / plain.h1Size),
        )
    }

    @Test
    fun theExportedMetricIsTheOneTheNativeTextIsSetIn() {
        for (fontScale in listOf(1f, 2f)) {
            show(measure(320f, fontScale))
            val line = composeRule.onNodeWithText(Specimen.CODE).bounds().height / density.density
            assertEquals("the code line at $fontScale", exported.typography.codeLine, line, 1f)
        }
    }

    @Test
    fun theExportDescribesTheSurfaceItWasTakenFrom() {
        show(measure(900f, fontScale = 1f))
        val geometry = exported.geometry
        val column = composeRule.onNodeWithTag(Specimen.COLUMN).bounds().width / density.density
        assertEquals(
            "the column and its padding are the measure",
            geometry.columnWidth,
            column + 2 * geometry.padding,
            1f,
        )
        val control = composeRule.onNodeWithText(APPEARANCE).bounds().height / density.density
        assertEquals("the target", geometry.touchTarget, control, 1f)
        assertEquals("the surface it was taken from", 900f, geometry.availableWidth, 0f)
    }

    @Test
    fun theControlIsNotSqueezedByANarrowMeasure() {
        show(measure(320f, fontScale = 2f))
        val narrow = control()
        show(measure(900f, fontScale = 2f))
        val wide = control()
        assertEquals("the label is written whole at either measure", wide.width, narrow.width, 2f)
        assertTrue(
            "the control keeps the touch floor: ${narrow.height}dp",
            narrow.height >= SlipboxTokens.Geometry.TOUCH_TARGET_DP - 1f,
        )
    }

    @Test
    fun aLongLineScrollsWithoutWideningTheColumn() {
        for (width in listOf(320f, 900f)) {
            show(measure(width, fontScale = 2f))
            val column = composeRule.onNodeWithTag(Specimen.COLUMN).bounds().width
            val blocks = listOf(Specimen.BLOCK to Specimen.CODE, Specimen.DISPLAY to Specimen.MATH)
            for ((tag, text) in blocks) {
                composeRule.onNodeWithTag(tag).performScrollTo()
                val block = composeRule.onNodeWithTag(tag).bounds()
                val before = composeRule.onNodeWithText(text).bounds()
                assertTrue("$width $tag: keeps the measure", block.width <= column + 1f)
                assertTrue("$width $tag: holds a longer line", before.width > block.width)
                composeRule.onNodeWithTag(tag).performTouchInput { swipeLeft() }
                composeRule.waitForIdle()
                assertTrue(
                    "$width $tag: scrolls its own line",
                    composeRule.onNodeWithText(text).bounds().left < before.left,
                )
                assertEquals(
                    "$width $tag: stays where it is",
                    block.width,
                    composeRule.onNodeWithTag(tag).bounds().width,
                    0f,
                )
            }
        }
    }

    private fun control(): Bounds = composeRule.onNodeWithText(APPEARANCE).bounds().dp(density)

    private fun measure(width: Float, fontScale: Float): VisualCase =
        VISUAL_CASES.first { it.width.value == width && !it.dark && it.fontScale == fontScale }

    private companion object {
        const val APPEARANCE = "Appearance"
    }
}
