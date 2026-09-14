/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.navigation

import android.os.Build
import androidx.activity.ComponentActivity
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.InputMode
import androidx.compose.ui.input.InputModeManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalInputModeManager
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertHeightIsAtLeast
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertIsOn
import androidx.compose.ui.test.assertWidthIsAtLeast
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.requestFocus
import androidx.compose.ui.unit.dp
import androidx.test.espresso.Espresso
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.SdkSuppress
import androidx.test.platform.app.InstrumentationRegistry
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.SlipboxApp
import io.github.b_vitamins.slipbox.ui.Evidence
import io.github.b_vitamins.slipbox.ui.MemoryStore
import io.github.b_vitamins.slipbox.ui.Record
import io.github.b_vitamins.slipbox.ui.RecordingContext
import io.github.b_vitamins.slipbox.ui.Specimen
import io.github.b_vitamins.slipbox.ui.frames
import io.github.b_vitamins.slipbox.ui.hex
import io.github.b_vitamins.slipbox.ui.paper
import io.github.b_vitamins.slipbox.ui.pixels
import io.github.b_vitamins.slipbox.ui.settings.ReadingPreferences
import io.github.b_vitamins.slipbox.ui.settings.ReadingPreferencesFile
import io.github.b_vitamins.slipbox.ui.settings.ReadingPreferencesRecord
import io.github.b_vitamins.slipbox.ui.settings.ReadingSettings
import io.github.b_vitamins.slipbox.ui.theme.SlipboxAppearance
import java.io.File
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
@SdkSuppress(minSdkVersion = Build.VERSION_CODES.O)
class NavigationChromeTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val back = context.getString(R.string.action_back)
    private val forward = context.getString(R.string.action_about)
    private val appearance = context.getString(R.string.action_appearance)
    private val reduceMotion = context.getString(R.string.appearance_reduce_motion)
    private val library = context.getString(R.string.library_empty)
    private val about = context.getString(R.string.about_license)

        private val directory = File(context.noBackupFilesDir, OWNED)
    private val record = File(directory, ReadingPreferencesFile.FILE_NAME)
    private val host = RecordingContext(context, directory)

    private lateinit var inputMode: InputModeManager
    private var owner: SlipboxBackStack? = null

    @Before
    @After
    fun emptyTheOwnedRecord() {
        directory.mkdirs()
        record.delete()
    }

    @Test
    fun theBackMarkOfASuppliedDestinationIsOneReachableGlyph() {
        show()
        open()

        val visible = composeRule.showing(back)
        val marks =
            composeRule
                .onAllNodesWithContentDescription(back, useUnmergedTree = true)
                .fetchSemanticsNodes()
                .size
        val mark =
            composeRule
                .onNodeWithContentDescription(back, useUnmergedTree = true)
                .assert(hasClickAction())
                .assertWidthIsAtLeast(TOUCH_TARGET)
                .assertHeightIsAtLeast(TOUCH_TARGET)
        Evidence.image(
            "navigation-reader-synthetic",
            composeRule.onNodeWithTag(Specimen.SURFACE).captureToImage(),
        )
        takeKeyboard()
        composeRule.onNodeWithContentDescription(back).requestFocus().assertIsFocused()
        Evidence.record(
            "navigation-back-mark",
            Record()
                .text("accessibleName", back)
                .count("marks", marks)
                .count("touchTargetDp", TOUCH_TARGET.value.toInt())
                .flag("visibleBackText", visible),
        )
        assertFalse("Back is a mark rather than a word", visible)
        assertEquals("one mark names Back", 1, marks)

        mark.performClick()
        composeRule.waitForIdle()
        assertEquals(listOf(SlipboxRoute.Start), history().entries)
    }

    @Test
    fun theAboutAndSettingsPathThisBuildHasIsStillReachedAndLeft() {
        recorded(ReadingPreferences(appearance = SlipboxAppearance.Light))
        showApp()

        composeRule.onNodeWithText(library).assertIsDisplayed()
        capture("navigation-library-light", dark = false)
        composeRule.onNodeWithText(forward).performClick()
        composeRule.waitForIdle()
        composeRule.onNodeWithText(about).assertIsDisplayed()
        capture("navigation-about-light", dark = false)

        composeRule.onNodeWithText(appearance).performClick()
        composeRule.waitForIdle()
        composeRule.onNodeWithText(reduceMotion).performClick()
        composeRule.waitForIdle()
        composeRule.onNodeWithText(reduceMotion).assertIsOn()
        Espresso.pressBack()
        composeRule.waitForIdle()
        composeRule.onNodeWithText(about).assertIsDisplayed()

        composeRule.onNodeWithContentDescription(back).performClick()
        composeRule.waitForIdle()
        composeRule.onNodeWithText(library).assertIsDisplayed()
        composeRule.onNodeWithContentDescription(back).assertDoesNotExist()
        assertEquals(
            "the choice taken along the way is in the record the next run reads",
            ReadingPreferences(appearance = SlipboxAppearance.Light, reduceMotion = true),
            stored(),
        )
    }

    @Test
    fun theChosenDarkSchemeIsWhatTheNativeChromeShows() {
        recorded(ReadingPreferences(appearance = SlipboxAppearance.Dark))
        showApp()

        composeRule.onNodeWithText(library).assertIsDisplayed()
        capture("navigation-library-dark", dark = true)
        composeRule.onNodeWithText(forward).performClick()
        composeRule.waitForIdle()
        composeRule.onNodeWithText(about).assertIsDisplayed()
        capture("navigation-about-dark", dark = true)
    }

    @Test
    fun aRouteThisBuildDoesNotPresentShowsNothing() {
        show()

        composeRule.runOnIdle {
            assertFalse(history().open(SlipboxRoute.Connection()))
            assertFalse(history().open(SlipboxRoute.SourceSettings(Synthetic.ALPHA)))
            assertFalse(history().open(SlipboxRoute.About))
        }
        composeRule.waitForIdle()

        assertEquals(listOf(SlipboxRoute.Start), history().entries)
        composeRule.onNodeWithText(Synthetic.LIBRARY).assertIsDisplayed()
        composeRule.onNodeWithText(about).assertDoesNotExist()
        composeRule.onNodeWithContentDescription(back).assertDoesNotExist()
    }

    @Test
    fun aReaderWhoRefusesMotionExchangesWithNoTransition() {
        show(ReadingSettings(MemoryStore(refusing)))

        composeRule.mainClock.autoAdvance = false
        composeRule.onNodeWithText(Synthetic.OPEN).performClick()
        val gone = composeRule.mainClock.frames { !composeRule.showing(Synthetic.LIBRARY) }
        assertTrue("the surface being left is gone at once: after $gone frame(s)", gone <= COMPOSED)

        composeRule.mainClock.autoAdvance = true
        composeRule.onNodeWithText(Specimen.TITLE).assertIsDisplayed()
    }

    private fun show(settings: ReadingSettings = ReadingSettings(MemoryStore())) {
        composeRule.setContent {
            inputMode = LocalInputModeManager.current
            SyntheticHost(settings, onOwner = { owner = it })
        }
        composeRule.waitForIdle()
    }

        private fun takeKeyboard() {
        composeRule.waitUntil(TIMEOUT) { composeRule.activity.hasWindowFocus() }
        assertTrue(
            "the surface leaves touch input",
            composeRule.runOnUiThread { inputMode.requestInputMode(InputMode.Keyboard) },
        )
    }

    private fun showApp() {
        composeRule.setContent { Hosted() }
        composeRule.waitForIdle()
    }

    @Composable
    private fun Hosted() {
        CompositionLocalProvider(LocalContext provides host) { SlipboxApp() }
    }

    private fun open() {
        composeRule.onNodeWithText(Synthetic.OPEN).performClick()
        composeRule.waitForIdle()
        composeRule.onNodeWithText(Specimen.TITLE).assertIsDisplayed()
    }

        private fun capture(name: String, dark: Boolean) {
        val image = composeRule.onRoot().captureToImage()
        val tone: Color = image.pixels()[EDGE, EDGE]
        Evidence.image(name, image)
        Evidence.record(name, Record().colour("canvas", tone).flag("dark", dark))
        assertEquals("$name is painted on its own page", hex(paper(dark)), hex(tone))
    }

    private fun recorded(preferences: ReadingPreferences) {
        assertNull(
            "the suite's own record was written",
            ReadingPreferencesFile(record).write(preferences),
        )
    }

    private fun stored(): ReadingPreferences? =
        (ReadingPreferencesFile(record).read() as? ReadingPreferencesRecord.Stored)?.preferences

    private fun history(): SlipboxBackStack = checkNotNull(owner) { "no destination presented" }

    private companion object {
        const val OWNED = "navigation-chrome"

        /** Allow framework scheduling, but less than one exchange. */
        const val COMPOSED = 5

        const val EDGE = 2

        const val TIMEOUT = 5_000L

        val TOUCH_TARGET = 48.dp

        val refusing =
            ReadingPreferencesRecord.Stored(ReadingPreferences(reduceMotion = true))
    }
}
