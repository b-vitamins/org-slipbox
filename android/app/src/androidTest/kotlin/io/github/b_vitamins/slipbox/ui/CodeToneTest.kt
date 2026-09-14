/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import android.os.Build
import androidx.activity.ComponentActivity
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PixelMap
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.IntRect
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.SdkSuppress
import io.github.b_vitamins.slipbox.ui.settings.ReadingSettings
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
@SdkSuppress(minSdkVersion = Build.VERSION_CODES.O)
class CodeToneTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    private lateinit var density: Density
    private var composed = false
    private var shown by mutableStateOf(CASES.first())

    @Test
    fun inlineAndBlockCodeAreSetOnThePageToneRatherThanTheCanvas() {
        for (case in CASES) {
            show(case)
            composeRule.onNodeWithTag(Specimen.BLOCK).performScrollTo()
            val page = paper(case.dark)
            val note = composeRule.onNodeWithTag(Specimen.SURFACE).captureToImage().pixels()
            val column = composeRule.onNodeWithTag(Specimen.COLUMN).bounds()
            val prose = composeRule.onNodeWithTag(Specimen.PROSE).bounds()
            val block = composeRule.onNodeWithTag(Specimen.BLOCK).bounds()

            // Sample the canvas between prose and code, clear of text.
            val between = ((prose.bottom + block.top) / 2).toInt()
            val canvas = note[column.left.toInt() - EDGE, between]
            assertEquals("${case.label}: the canvas", hex(surface(case.dark)), hex(canvas))

            // Exclude the border and rounded corners.
            val image = composeRule.onNodeWithTag(Specimen.BLOCK).captureToImage()
            val pixels = image.pixels()
            val inside =
                IntRect(0, 0, pixels.width, pixels.height)
                    .deflate(SlipboxDimensions.blockCorner.pixels(density).toInt() + 1)
            val blockTone = pixels[inside.left, (inside.top + inside.bottom) / 2]
            assertEquals("${case.label}: the block's tone", hex(page), hex(blockTone))
            assertNotEquals(
                "${case.label}: the block does not share the canvas",
                hex(canvas),
                hex(blockTone),
            )

            val paragraph = prose.rows(note)
            val fragment = note.where(page, paragraph)
            assertTrue(
                "${case.label}: an inline fragment is set on the page tone as well",
                fragment.width > 0 && fragment.height > 0,
            )
            assertTrue(
                "${case.label}: a fragment of a line rather than the paragraph: $fragment",
                fragment.width < paragraph.width && fragment.height < paragraph.height,
            )
            val inlineTone = note[fragment.left, (fragment.top + fragment.bottom) / 2]
            assertEquals("${case.label}: the fragment's tone", hex(page), hex(inlineTone))

            val blockReads = pixels.reads(inside)
            val inlineReads = note.reads(fragment)
            assertTrue("${case.label}: the source line reads: $blockReads", blockReads >= CONTRAST)
            assertTrue("${case.label}: the fragment reads: $inlineReads", inlineReads >= CONTRAST)

            Evidence.image("native-code-${case.label}", image)
            Evidence.record(
                "native-code-${case.label}",
                Record()
                    .text("label", case.label)
                    .colour("block", blockTone)
                    .colour("inline", inlineTone)
                    .colour("canvas", canvas)
                    .size("blockReads", blockReads)
                    .size("inlineReads", inlineReads)
                    .count("fragmentWidthPx", fragment.width)
                    .count("fragmentHeightPx", fragment.height),
            )
        }
    }

    private fun show(case: VisualCase) {
        if (composed) {
            composeRule.runOnIdle { shown = case }
        } else {
            shown = case
            composeRule.setContent {
                shown.Content {
                    density = LocalDensity.current
                    key(shown.label) {
                        SpecimenNote(settings = remember { ReadingSettings(MemoryStore()) })
                    }
                }
            }
            composed = true
        }
        composeRule.waitForIdle()
    }

    private fun Bounds.rows(pixels: PixelMap): IntRect =
        IntRect(
            left = left.toInt().coerceAtLeast(0),
            top = top.toInt().coerceAtLeast(0),
            right = right.toInt().coerceAtMost(pixels.width),
            bottom = bottom.toInt().coerceAtMost(pixels.height),
        )

    private fun PixelMap.where(tone: Color, region: IntRect): IntRect {
        var left = region.right
        var top = region.bottom
        var right = region.left
        var bottom = region.top
        for (y in region.top until region.bottom) {
            for (x in region.left until region.right) {
                if (this[x, y] != tone) continue
                left = minOf(left, x)
                top = minOf(top, y)
                right = maxOf(right, x + 1)
                bottom = maxOf(bottom, y + 1)
            }
        }
        return if (right > left && bottom > top) IntRect(left, top, right, bottom) else IntRect.Zero
    }

    private fun PixelMap.reads(region: IntRect): Float =
        contrast(
            darkest(region.left, region.top, region.right, region.bottom),
            lightest(region.left, region.top, region.right, region.bottom),
        )

    private companion object {
        const val EDGE = 2

        const val CONTRAST = 7f

        val CASES = VISUAL_CASES.filter { it.fontScale == 1f }
    }
}
