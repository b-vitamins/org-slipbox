/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.document

import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import io.github.b_vitamins.slipbox.ui.theme.SlipboxMotion
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTokens
import java.util.Locale
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class DocumentPresentationTest {

    private val locale = Locale.getDefault()

    @After
    fun restoreLocale() {
        Locale.setDefault(locale)
    }

    @Test
    fun anUnscaledReaderGetsTheSurfacesOwnScale() {
        val type = presentation(density = 3f, fontScale = 1f).typography
        assertEquals(SlipboxTokens.Type.BODY_SIZE_SP, type.bodySize, TOLERANCE)
        assertEquals(SlipboxTokens.Type.BODY_LINE_SP, type.bodyLine, TOLERANCE)
        assertEquals(SlipboxTokens.Type.H1_SIZE_SP, type.h1Size, TOLERANCE)
        assertEquals(SlipboxTokens.Type.H2_SIZE_SP, type.h2Size, TOLERANCE)
        assertEquals(SlipboxTokens.Type.CODE_SIZE_SP, type.codeSize, TOLERANCE)
        assertEquals(SlipboxTokens.Type.CHROME_SIZE_SP, type.chromeSize, TOLERANCE)
        assertEquals(SlipboxTokens.Type.TABLE_SIZE_SP, type.tableSize, TOLERANCE)
        assertEquals(SlipboxTokens.Type.LETTER_SPACING_EM, type.letterSpacingEm, 0f)
    }

    /** A CSS pixel is a density-independent pixel: the density must cancel out. */
    @Test
    fun theDeviceDensityDoesNotReachTheDocument() {
        for (density in listOf(1f, 1.5f, 2.625f, 4f)) {
            val type = presentation(density = density, fontScale = 1f).typography
            assertEquals(
                "density $density",
                SlipboxTokens.Type.BODY_SIZE_SP,
                type.bodySize,
                TOLERANCE,
            )
        }
    }

    /**
     * The platform's font scale is not a multiplication: text grows, by less than the
     * scale, and the smallest text gains the most. The document must be handed that
     * curve's answer rather than a scale to apply itself.
     */
    @Test
    fun aScaledReaderGetsTheCurvesAnswerAndNotTheScale() {
        val plain = presentation(density = 3f, fontScale = 1f).typography
        val scaled = presentation(density = 3f, fontScale = 2f).typography
        assertTrue(scaled.bodySize > plain.bodySize)
        assertTrue(scaled.bodySize < 2 * plain.bodySize)
        assertTrue(scaled.codeSize > plain.codeSize)
        assertTrue(scaled.h1Size > plain.h1Size)
        assertTrue(
            "small text gains more than large",
            scaled.codeSize / plain.codeSize > scaled.h1Size / plain.h1Size,
        )
    }

    /**
     * A line height is set as the ratio it bears to its own size, and the curve does not
     * preserve that ratio when a length passes through it. Text is laid out with the
     * ratio, so an export that hands over a scaled length describes no line box.
     */
    @Test
    fun aScaledLineKeepsTheRatioItsOwnSizeIsSetIn() {
        for (fontScale in listOf(1f, 1.3f, 2f)) {
            val type = presentation(density = 3f, fontScale = fontScale).typography
            keepsItsRatio(
                "the reading line at $fontScale",
                SlipboxTokens.Type.BODY_SIZE_SP,
                SlipboxTokens.Type.BODY_LINE_SP,
                type.bodySize,
                type.bodyLine,
            )
            keepsItsRatio(
                "the title line at $fontScale",
                SlipboxTokens.Type.H1_SIZE_SP,
                SlipboxTokens.Type.H1_LINE_SP,
                type.h1Size,
                type.h1Line,
            )
            keepsItsRatio(
                "the heading line at $fontScale",
                SlipboxTokens.Type.H2_SIZE_SP,
                SlipboxTokens.Type.H2_LINE_SP,
                type.h2Size,
                type.h2Line,
            )
            keepsItsRatio(
                "the code line at $fontScale",
                SlipboxTokens.Type.CODE_SIZE_SP,
                SlipboxTokens.Type.CODE_LINE_SP,
                type.codeSize,
                type.codeLine,
            )
        }
    }

    private fun keepsItsRatio(
        label: String,
        sizeSp: Float,
        lineSp: Float,
        size: Float,
        line: Float,
    ) {
        assertEquals(label, lineSp / sizeSp, line / size, TOLERANCE)
        assertTrue("$label grows with its size", line >= lineSp)
    }

    @Test
    fun aScaledReaderGetsOneAnswerAtEveryDensity() {
        val coarse = presentation(density = 1f, fontScale = 2f).typography
        val fine = presentation(density = 4f, fontScale = 2f).typography
        assertEquals(coarse.bodySize, fine.bodySize, TOLERANCE)
        assertEquals(coarse.codeSize, fine.codeSize, TOLERANCE)
    }

    @Test
    fun theMeasureIsTheSameMeasureAtEveryScale() {
        for (fontScale in listOf(1f, 1.3f, 2f)) {
            val geometry = presentation(density = 3f, fontScale = fontScale).geometry
            assertEquals(SlipboxTokens.Geometry.READING_MEASURE_DP, geometry.columnWidth, 0f)
            assertEquals(SlipboxTokens.Geometry.READING_PADDING_DP, geometry.padding, 0f)
            assertEquals(SlipboxTokens.Geometry.TOUCH_TARGET_DP, geometry.touchTarget, 0f)
            assertEquals(320f, geometry.availableWidth, 0f)
        }
    }

    @Test
    fun theSchemeIsResolvedBeforeTheDocumentIsToldIt() {
        assertEquals(DocumentTheme.Light, presentation(dark = false).theme)
        assertEquals(DocumentTheme.Dark, presentation(dark = true).theme)
        assertEquals("light", DocumentTheme.Light.option)
        assertEquals("dark", DocumentTheme.Dark.option)
    }

    @Test
    fun anUnscaledPlatformHandsOverTheCanonicalDurations() {
        val motion = presentation().motion
        assertEquals(SlipboxTokens.Motion.COLUMN_MS, motion.columnMs)
        assertEquals(SlipboxTokens.Motion.SHADOW_MS, motion.shadowMs)
        assertEquals(SlipboxTokens.Motion.OPACITY_MS, motion.opacityMs)
        assertEquals(SlipboxTokens.Motion.CROSSFADE_MS, motion.crossfadeMs)
        assertFalse(motion.reduced)
    }

    @Test
    fun aScaledPlatformReachesTheDocumentThroughTheHost() {
        val motion = presentation(motion = SlipboxMotion(platformScale = 2f)).motion
        assertEquals(2 * SlipboxTokens.Motion.COLUMN_MS, motion.columnMs)
        assertEquals(2 * SlipboxTokens.Motion.CROSSFADE_MS, motion.crossfadeMs)
        assertFalse(motion.reduced)
    }

    @Test
    fun aReaderWhoAsksForLessMotionGetsADocumentWithoutIt() {
        val refusals =
            listOf(SlipboxMotion(reduceMotion = true), SlipboxMotion(platformScale = 0f))
        for (motion in refusals) {
            val document = presentation(motion = motion).motion
            assertTrue(document.reduced)
            assertEquals(0, document.columnMs)
            assertEquals(0, document.shadowMs)
            assertEquals(0, document.opacityMs)
            assertEquals(0, document.crossfadeMs)
            assertEquals("0ms", presentation(motion = motion).cssVariables()["--dur-column"])
        }
    }

    @Test
    fun theExportedPropertiesAreTheOnesTheSurfaceReads() {
        val variables = presentation().cssVariables()
        assertEquals(EXPORTED, variables.keys.toList())
        assertEquals("17.00px", variables["--text-size"])
        assertEquals("24.00px", variables["--text-line"])
        assertEquals("-0.0024em", variables["--text-letter-spacing"])
        assertEquals("13.00px", variables["--code-size"])
        assertEquals("12.00px", variables["--slipbox-chrome-size"])
        assertEquals("15.00px", variables["--slipbox-table-size"])
        assertEquals("625.00px", variables["--column-width"])
        assertEquals("16.00px", variables["--note-padding"])
        assertEquals("48.00px", variables["--touch-target"])
        assertEquals("200ms", variables["--dur-column"])
        assertEquals("75ms", variables["--dur-opacity"])
    }

    @Test
    fun anExportSaysWhatItPaintsAndHowItMoves() {
        val json = presentation(dark = true).toJson()
        assertTrue(json, json.contains("\"theme\":\"dark\""))
        assertTrue(json, json.contains("\"bodySize\":17.00"))
        assertTrue(json, json.contains("\"letterSpacingEm\":-0.0024"))
        assertTrue(json, json.contains("\"columnWidth\":625.00"))
        assertTrue(json, json.contains("\"availableWidth\":320.00"))
        assertTrue(json, json.contains("\"columnMs\":200"))
        assertTrue(json, json.contains("\"reduced\":false"))
        assertTrue(json, json.contains("\"--text-size\":\"17.00px\""))
        assertEquals(
            "every object the export opens is closed",
            json.count { it == '{' },
            json.count { it == '}' },
        )
    }

    /** A locale that writes its decimals with a comma must not reach the export. */
    @Test
    fun anExportReadsTheSameInEveryLocale() {
        val exported = presentation().toJson()
        Locale.setDefault(Locale.forLanguageTag("de-DE"))
        assertEquals(exported, presentation().toJson())
        assertTrue(exported.contains("\"--text-size\":\"17.00px\""))
    }

    private fun presentation(
        density: Float = 3f,
        fontScale: Float = 1f,
        dark: Boolean = false,
        motion: SlipboxMotion = SlipboxMotion(),
    ): DocumentPresentation =
        documentPresentation(
            density = Density(density, fontScale),
            dark = dark,
            motion = motion,
            availableWidth = 320.dp,
        )

    private companion object {
        const val TOLERANCE = 0.001f
        val EXPORTED =
            listOf(
                "--text-size",
                "--text-line",
                "--text-letter-spacing",
                "--h1-size",
                "--h1-line",
                "--h2-size",
                "--h2-line",
                "--code-size",
                "--code-line",
                "--slipbox-chrome-size",
                "--slipbox-table-size",
                "--column-width",
                "--note-padding",
                "--touch-target",
                "--dur-column",
                "--dur-shadow",
                "--dur-opacity",
                "--dur-crossfade",
            )
    }
}
