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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.unit.Density
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.SdkSuppress
import androidx.test.platform.app.InstrumentationRegistry
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.ui.settings.ReadingSettings
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTokens
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The surfaces the app ships, rather than the specimen they are measured with: what a
 * reader opens, and what they reach it in either scheme.
 */
@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
@SdkSuppress(minSdkVersion = Build.VERSION_CODES.O)
class OrdinarySurfacesTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val library = context.getString(R.string.app_name)
    private val empty = context.getString(R.string.library_empty)
    private val about = context.getString(R.string.action_about)
    private val version = context.getString(R.string.about_label_version)
    private val back = context.getString(R.string.action_back)

    private lateinit var density: Density
    private var shown by mutableStateOf(LIGHT)
    private var atAbout by mutableStateOf(false)

    @Test
    fun theLibraryIsCapturedAsAReaderFindsIt() {
        show()
        composeRule.onNodeWithText(library).assertIsDisplayed()
        composeRule.onNodeWithText(empty).assertIsDisplayed()
        val target = composeRule.onNodeWithText(about).bounds().height / density.density
        assertTrue(
            "the way on keeps the touch floor: ${target}dp",
            target >= SlipboxTokens.Geometry.TOUCH_TARGET_DP - 1f,
        )
        capture("ordinary-library-light")
    }

    @Test
    fun theAboutSurfaceIsCapturedInEitherScheme() {
        show()
        composeRule.onNodeWithText(about).performClick()
        composeRule.waitForIdle()
        composeRule.onNodeWithContentDescription(back).assertIsDisplayed()
        composeRule.onNodeWithText(version).assertIsDisplayed()
        val light = capture("ordinary-about-light")
        assertEquals("the light canvas", hex(paper(dark = false)), hex(light))

        composeRule.runOnIdle { shown = DARK }
        composeRule.waitForIdle()
        composeRule.onNodeWithText(version).assertIsDisplayed()
        val dark = capture("ordinary-about-dark")
        assertEquals("the dark canvas", hex(paper(dark = true)), hex(dark))
    }

    private fun show() {
        composeRule.setContent {
            shown.Content {
                density = LocalDensity.current
                val settings = remember { ReadingSettings(MemoryStore()) }
                if (atAbout) {
                    AboutScreen(onBack = { atAbout = false }, settings = settings)
                } else {
                    LibraryScreen(onOpenAbout = { atAbout = true })
                }
            }
        }
        composeRule.waitForIdle()
    }

    /** Writes one surface as it stands, and answers with the canvas it stands on. */
    private fun capture(name: String): Color {
        val image: ImageBitmap = composeRule.onRoot().captureToImage()
        Evidence.image(name, image)
        val pixels = image.pixels()
        val canvas = pixels[2, 2]
        Evidence.record(
            name,
            Record()
                .colour("canvas", canvas)
                .size("density", density.density)
                .size("widthDp", pixels.width / density.density)
                .size("heightDp", pixels.height / density.density),
        )
        return canvas
    }

    private companion object {
        val LIGHT = VISUAL_CASES.first { !it.dark && it.fontScale == 1f }
        val DARK = VISUAL_CASES.first { it.dark && it.fontScale == 1f }
    }
}
