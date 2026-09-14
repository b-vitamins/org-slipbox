/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import android.os.Build
import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.PixelMap
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.unit.Density
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.SdkSuppress
import androidx.test.platform.app.InstrumentationRegistry
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.ui.document.DocumentPresentation
import io.github.b_vitamins.slipbox.ui.document.rememberDocumentPresentation
import io.github.b_vitamins.slipbox.ui.settings.ReadingSettings
import io.github.b_vitamins.slipbox.ui.theme.SlipboxAppearance
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTokens
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The specimen at every measure, scheme and reader's scale, captured beside the
 * measurements the capture was taken with and the presentation a document would have
 * been mounted with on the same surface.
 */
@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
@SdkSuppress(minSdkVersion = Build.VERSION_CODES.O)
class VisualMatrixTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val back = context.getString(R.string.action_back)
    private val control = context.getString(R.string.action_appearance)
    private val paneTitle = context.getString(R.string.appearance_title)

    private lateinit var density: Density
    private lateinit var exported: DocumentPresentation
    private var footInset = 0
    private var composed = false
    private var shown by mutableStateOf(VISUAL_CASES.first())

    @Test
    fun everyCellPaintsItsSchemeAtItsMeasureAndIsCapturedWithIt() {
        for (case in VISUAL_CASES) {
            show(case)
            val image = surfaceImage()
            val pixels = image.pixels()
            Evidence.image("native-${case.label}", image)
            Evidence.record(
                "presentation-${case.label}",
                Record().raw("presentation", exported.toJson()),
            )
            Evidence.record(
                "native-${case.label}",
                Record()
                    .text("label", case.label)
                    .size("density", density.density)
                    .size("fontScale", case.fontScale)
                    .count("widthPx", pixels.width)
                    .count("heightPx", pixels.height)
                    .size("widthDp", pixels.width / density.density)
                    .size("heightDp", pixels.height / density.density)
                    .colour("canvas", canvas(case, pixels))
                    .nested("column", column(case))
                    .nested("rule", rule(case, pixels))
                    .nested("contrast", contrasts(case)),
            )
        }
    }

    @Test
    fun theRevealIsCapturedOverTheSurfaceItDims() {
        for (case in listOf(VISUAL_CASES.first(), VISUAL_CASES.last())) {
            show(case)
            composeRule.onNodeWithText(control).performClick()
            composeRule.waitForIdle()
            val sheet =
                composeRule
                    .onNode(SemanticsMatcher.expectValue(SemanticsProperties.PaneTitle, paneTitle))
                    .assertIsDisplayed()
                    .bounds()
            val image = surfaceImage()
            val pixels = image.pixels()
            val dimmed = pixels[2, 2]
            Evidence.image("native-reveal-${case.label}", image)

            assertTrue(
                "${case.label}: the surface behind the reveal is dimmed",
                dimmed.luminance() < paper(case.dark).luminance(),
            )
            assertEquals(
                "${case.label}: the reveal keeps the measure",
                minOf(case.width.value, SlipboxTokens.Geometry.READING_MEASURE_DP),
                sheet.width / density.density,
                1f,
            )
            assertTrue(
                "${case.label}: the reveal rises from the edge it was opened at",
                sheet.top > 0f,
            )
            assertEquals(
                "${case.label}: and rests against it",
                pixels.height.toFloat(),
                sheet.bottom,
                1f,
            )
            Evidence.record(
                "native-reveal-${case.label}",
                Record()
                    .colour("paper", paper(case.dark))
                    .colour("dimmed", dimmed)
                    .size("revealTopPx", sheet.top)
                    .size("revealHeightPx", sheet.height)
                    .size("revealWidthDp", sheet.width / density.density),
            )
        }
    }

    /**
     * One cell of the matrix, in its own subtree: a cell begins at the top of the note
     * rather than where the cell before it was left.
     */
    private fun show(case: VisualCase) {
        if (composed) {
            composeRule.runOnIdle { shown = case }
        } else {
            shown = case
            composeRule.setContent {
                shown.Content {
                    density = LocalDensity.current
                    footInset = WindowInsets.safeDrawing.getBottom(density)
                    exported =
                        rememberDocumentPresentation(
                            appearance = SlipboxAppearance.System,
                            reduceMotion = false,
                            availableWidth = shown.width,
                        )
                    key(shown.label) {
                        SpecimenNote(settings = remember { ReadingSettings(MemoryStore()) })
                    }
                }
            }
            composed = true
        }
        composeRule.waitForIdle()
    }

    private fun surfaceImage(): ImageBitmap =
        composeRule.onNodeWithTag(Specimen.SURFACE).captureToImage()

    /**
     * The tone a note is set on, which is the tone a document mounts on: read inside
     * the column, in the padding no line reaches, at the head of the note and again at
     * the last row the column is given, where a short note must not end in a seam.
     */
    private fun canvas(case: VisualCase, pixels: PixelMap): Color {
        val inside = composeRule.onNodeWithTag(Specimen.COLUMN).bounds()
        val x = inside.left.toInt() - EDGE
        val painted = pixels[x, inside.top.toInt() + EDGE]
        assertEquals("${case.label}: the canvas", hex(surface(case.dark)), hex(painted))
        assertEquals(
            "${case.label}: the canvas holds to the foot of the reading area",
            hex(surface(case.dark)),
            hex(pixels[x, pixels.height - footInset - EDGE]),
        )
        return painted
    }

    /** The row the column begins on, which its own padding sets the note in from. */
    private fun columnTop(): Int =
        (composeRule.onNodeWithTag(Specimen.COLUMN).bounds().top -
                SlipboxDimensions.readingPadding.pixels(density))
            .toInt()

    /** The measure the column keeps, which the reader's scale does not widen. */
    private fun column(case: VisualCase): Record {
        val measure =
            minOf(case.width.value, SlipboxTokens.Geometry.READING_MEASURE_DP) -
                2 * SlipboxTokens.Geometry.READING_PADDING_DP
        val laid = composeRule.onNodeWithTag(Specimen.COLUMN).bounds().width / density.density
        assertEquals("${case.label}: the measure", measure, laid, 1f)
        val target = composeRule.onNodeWithText(control).bounds().height / density.density
        assertTrue(
            "${case.label}: the control keeps the touch floor: ${target}dp",
            target >= SlipboxTokens.Geometry.TOUCH_TARGET_DP - 1f,
        )
        return Record()
            .size("measureDp", laid)
            .size("targetDp", target)
            .size("exportedColumnWidth", exported.geometry.columnWidth)
    }

    /** The single rule that separates the header: where it falls, and how quietly. */
    private fun rule(case: VisualCase, pixels: PixelMap): Record {
        val top = columnTop()
        val runs = pixels.runsDown(x = EDGE, background = paper(case.dark), until = top)
        assertEquals("${case.label}: one rule crosses the page above the column", 1, runs.size)
        val painted = runs.single()
        val thickness = painted.last - painted.first + 1
        val hairline = SlipboxDimensions.hairline.pixels(density)
        assertTrue(
            "${case.label}: a hairline and not a band: ${thickness}px of ${hairline}px",
            thickness <= hairline + 2f,
        )
        val header =
            maxOf(
                composeRule.onNodeWithContentDescription(back).bounds().bottom,
                composeRule.onNodeWithText(Specimen.TITLE).bounds().bottom,
                composeRule.onNodeWithText(control).bounds().bottom,
            )
        assertEquals(
            "${case.label}: the rule follows the header it closes",
            header + SlipboxDimensions.headerPaddingVertical.pixels(density),
            painted.first.toFloat(),
            2f,
        )
        val tone = pixels[EDGE, (painted.first + painted.last) / 2]
        val against = contrast(tone, paper(case.dark))
        assertTrue("${case.label}: the rule stays quieter than the type: $against", against < QUIET)
        // Under the rule the narrow measure is the column itself; the wide one is its page.
        val beside = pixels[EDGE, top + EDGE]
        val edge =
            if (case.width.value > SlipboxTokens.Geometry.READING_MEASURE_DP) {
                paper(case.dark)
            } else {
                surface(case.dark)
            }
        assertEquals("${case.label}: beside the column", hex(edge), hex(beside))
        return Record()
            .count("thicknessPx", thickness)
            .size("hairlinePx", hairline)
            .size("topPx", painted.first.toFloat())
            .colour("tone", tone)
            .size("againstThePage", against)
            .colour("beside", beside)
    }

    /** The contrast a reader meets the type with, measured off the type as painted. */
    private fun contrasts(case: VisualCase): Record {
        val ink = measuredContrast(Specimen.HEAD)
        assertTrue("${case.label}: the heading reads: $ink", ink >= INK_CONTRAST)
        composeRule.onNodeWithTag(Specimen.DISPLAY).performScrollTo()
        val muted = measuredContrast(Specimen.DISPLAY)
        assertTrue("${case.label}: the display math reads: $muted", muted >= MUTED_CONTRAST)
        return Record().size("ink", ink).size("muted", muted)
    }

    /** The contrast between the extreme tones one node was actually painted with. */
    private fun measuredContrast(tag: String): Float {
        val pixels = composeRule.onNodeWithTag(tag).captureToImage().pixels()
        return contrast(
            pixels.darkest(0, 0, pixels.width, pixels.height),
            pixels.lightest(0, 0, pixels.width, pixels.height),
        )
    }

    private companion object {
        /** A column of pixels beside the reading column, where only a rule crosses. */
        const val EDGE = 2

        const val INK_CONTRAST = 7f
        const val MUTED_CONTRAST = 4.5f
        const val QUIET = 3f
    }
}
