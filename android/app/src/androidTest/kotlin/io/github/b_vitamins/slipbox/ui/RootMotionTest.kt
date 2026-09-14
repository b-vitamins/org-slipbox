/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import android.os.Build
import android.provider.Settings
import androidx.activity.ComponentActivity
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsOn
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.junit4.StateRestorationTester
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performSemanticsAction
import androidx.test.espresso.Espresso
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.SdkSuppress
import androidx.test.platform.app.InstrumentationRegistry
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.SlipboxApp
import io.github.b_vitamins.slipbox.ui.settings.ReadingPreferences
import io.github.b_vitamins.slipbox.ui.settings.ReadingPreferencesFile
import io.github.b_vitamins.slipbox.ui.settings.ReadingPreferencesRecord
import io.github.b_vitamins.slipbox.ui.theme.SlipboxAppearance
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTokens
import io.github.b_vitamins.slipbox.ui.theme.rememberPlatformMotionScale
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

/**
 * Measure crossfade completion by the outgoing surface leaving composition.
 */
@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
@SdkSuppress(minSdkVersion = Build.VERSION_CODES.Q)
class RootMotionTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val context = instrumentation.targetContext

    private val forward = context.getString(R.string.action_about)
    private val back = context.getString(R.string.action_back)
    private val appearance = context.getString(R.string.action_appearance)
    private val library = context.getString(R.string.library_empty)
    private val about = context.getString(R.string.about_license)
    private val reduceMotion = context.getString(R.string.appearance_reduce_motion)
    private val chooseDark = context.getString(R.string.appearance_dark)
    private val paneTitle = context.getString(R.string.appearance_title)

    private val directory = File(context.noBackupFilesDir, OWNED)
    private val record = File(directory, ReadingPreferencesFile.FILE_NAME)
    private val host = RecordingContext(context, directory)

    private var previousScale: String? = null
    private var platformScale = -1f

    @Before
    fun adoptTheShellIdentityAndEmptyTheOwnedRecord() {
        instrumentation.uiAutomation.adoptShellPermissionIdentity()
        previousScale =
            Settings.Global.getString(
                context.contentResolver,
                Settings.Global.ANIMATOR_DURATION_SCALE,
            )
        directory.mkdirs()
        record.delete()
    }

    @After
    fun restoreTheSettingDropTheIdentityAndDiscardTheOwnedRecord() {
        Settings.Global.putString(
            context.contentResolver,
            Settings.Global.ANIMATOR_DURATION_SCALE,
            previousScale,
        )
        instrumentation.uiAutomation.dropShellPermissionIdentity()
        record.delete()
    }

    @Test
    fun theRootExchangeTakesTheSurfacesOwnDurationAndComesToRest() {
        scale(1f)
        show()
        composeRule.onNodeWithText(library).assertIsDisplayed()

        composeRule.mainClock.autoAdvance = false
        composeRule.onNodeWithText(forward).performClick()
        val bothUp = composeRule.mainClock.frames { showing(library) && showing(about) }
        composeRule.mainClock.advanceTimeBy(HALFWAY)
        val halfway = showing(library)
        composeRule.mainClock.advanceTimeBy(ONWARD)
        val settled = !showing(library)
        composeRule.mainClock.advanceTimeBy(RESTED)
        val rested = !showing(library)

        assertTrue(
            "both surfaces are up while they exchange: after $bothUp frame(s)",
            bothUp <= COMPOSED,
        )
        assertTrue("the surface being left is still up halfway through", halfway)
        assertTrue("it is gone once the exchange has had its own duration", settled)
        assertTrue("and nothing afterwards puts it back", rested)
        composeRule.onNodeWithText(about).assertIsDisplayed()
        Evidence.record(
            "root-exchange",
            Record()
                .count("crossfadeMs", SlipboxTokens.Motion.CROSSFADE_MS)
                .count("bothUpAfterFrames", bothUp)
                .count("halfwayMs", HALFWAY.toInt())
                .count("measuredAtMs", (HALFWAY + ONWARD).toInt())
                .flag("leftSurfaceStillUpHalfway", halfway)
                .flag("leftSurfaceGoneOnceSettled", settled),
        )
    }

    @Test
    fun thePlatformsScaleIsNotAppliedASecondTimeToTheRootExchange() {
        scale(2f)
        show()
        assertEquals("the platform's scale reaches the surfaces", 2f, platformScale, 0f)

        composeRule.mainClock.autoAdvance = false
        composeRule.onNodeWithText(forward).performClick()
        composeRule.mainClock.advanceTimeByFrame()
        composeRule.mainClock.advanceTimeBy(HALFWAY)
        val halfway = showing(library)
        composeRule.mainClock.advanceTimeBy(ONWARD)
        val settled = !showing(library)

        assertTrue("the surface being left is still up halfway through", halfway)
        assertTrue("and gone by the surface's own duration rather than twice it", settled)
        Evidence.record(
            "root-exchange-scaled",
            Record()
                .size("platformScale", platformScale)
                .count("crossfadeMs", SlipboxTokens.Motion.CROSSFADE_MS)
                .count("measuredAtMs", (HALFWAY + ONWARD).toInt())
                .flag("leftSurfaceGoneOnceSettled", settled),
        )
    }

    @Test
    fun aReaderWhoRefusesMotionExchangesWithNoTransitionAtAll() {
        scale(1f)
        recorded(ReadingPreferences(reduceMotion = true))
        show()

        composeRule.mainClock.autoAdvance = false
        composeRule.onNodeWithText(forward).performClick()
        goneAtOnce("the surface being left is gone at once", library)
        composeRule.onNodeWithText(about).assertIsDisplayed()

        composeRule.mainClock.autoAdvance = true
        composeRule.onNodeWithText(appearance).performClick()
        composeRule.waitForIdle()
        composeRule.onNodeWithText(reduceMotion).assertIsOn()
    }

    @Test
    fun aPlatformThatRefusesMotionExchangesWithNoTransitionAtAll() {
        scale(0f)
        show()
        assertEquals("the platform's refusal reaches the surfaces", 0f, platformScale, 0f)

        composeRule.mainClock.autoAdvance = false
        composeRule.onNodeWithText(forward).performClick()
        goneAtOnce("the surface being left is gone at once", library)
        composeRule.onNodeWithText(about).assertIsDisplayed()
    }

    @Test
    fun aRefusalThatArrivesWhileAnExchangeIsOutstandingGovernsTheNextOne() {
        scale(1f)
        show()

        composeRule.mainClock.autoAdvance = false
        composeRule.onNodeWithText(forward).performClick()
        composeRule.mainClock.advanceTimeByFrame()
        assertTrue("the exchange is still outstanding", showing(library))
        scale(0f)
        composeRule.mainClock.advanceTimeBy(HALFWAY + ONWARD)
        assertFalse("it finished rather than lingering", showing(library))
        composeRule.onNodeWithText(about).assertIsDisplayed()

        composeRule.mainClock.autoAdvance = true
        composeRule.waitUntil(TIMEOUT) { platformScale == 0f }
        composeRule.mainClock.autoAdvance = false
        composeRule.onNodeWithContentDescription(back).performClick()
        goneAtOnce("and the exchange after it carries none at all", about)
        composeRule.onNodeWithText(library).assertIsDisplayed()
    }

    @Test
    fun aChoiceMadeWhileARevealIsMovingLeavesItSettledAndGovernsTheNext() {
        scale(1f)
        show()
        open()

        composeRule.mainClock.autoAdvance = false
        composeRule.onNodeWithText(appearance).performClick()
        composeRule.mainClock.advanceTimeByFrame()
        val start = sheetTop()
        composeRule.mainClock.advanceTimeBy(STEP)
        val moving = sheetTop()
        // Invoke semantics while the row is still moving, independently of its position.
        composeRule.onNodeWithText(reduceMotion).performSemanticsAction(SemanticsActions.OnClick)
        composeRule.mainClock.advanceTimeBy(STEP)
        val after = sheetTop()
        composeRule.mainClock.advanceTimeBy(RESTED)
        val rest = sheetTop()
        assertTrue("the reveal reached its place: $start of $rest", start > rest)
        assertTrue("moving towards it: $moving of $rest..$start", moving in rest..start)
        assertTrue("and still towards it after the choice: $after", after in rest..moving)

        composeRule.mainClock.autoAdvance = true
        Espresso.pressBack()
        composeRule.waitForIdle()
        composeRule.mainClock.autoAdvance = false
        composeRule.onNodeWithText(appearance).performClick()
        val placed = composeRule.mainClock.frames { revealed() && sheetTop() <= rest + EDGE }
        assertEquals("the next reveal is in its place already", rest, sheetTop(), EDGE)
        assertTrue("and was in it at once: after $placed frame(s)", placed <= COMPOSED)
        composeRule.onNodeWithText(reduceMotion).assertIsOn()
    }

    @Test
    fun aChoiceRecordedInTheRevealGovernsTheNextRootExchange() {
        scale(1f)
        show()
        open()
        composeRule.onNodeWithText(appearance).performClick()
        composeRule.waitForIdle()
        composeRule.onNodeWithText(reduceMotion).performClick()
        composeRule.waitForIdle()
        composeRule.onNodeWithText(reduceMotion).assertIsOn()
        Espresso.pressBack()
        composeRule.waitForIdle()

        composeRule.mainClock.autoAdvance = false
        composeRule.onNodeWithContentDescription(back).performClick()
        goneAtOnce("the surface being left is gone at once", about)
        composeRule.onNodeWithText(library).assertIsDisplayed()
        assertEquals(
            "and the choice is in the record the next run reads",
            ReadingPreferences(reduceMotion = true),
            stored(),
        )
    }

    @Test
    fun aRecordedChoiceSurvivesTheAppBeingRecreated() {
        scale(1f)
        val restoration = StateRestorationTester(composeRule)
        restoration.setContent { Hosted() }
        composeRule.waitForIdle()
        open()
        composeRule.onNodeWithText(appearance).performClick()
        composeRule.waitForIdle()
        composeRule.onNodeWithText(chooseDark).performClick()
        composeRule.waitForIdle()
        Espresso.pressBack()
        composeRule.waitForIdle()

        restoration.emulateSavedInstanceStateRestore()
        composeRule.waitForIdle()

        composeRule.onNodeWithText(about).assertIsDisplayed()
        composeRule.onNodeWithText(appearance).performClick()
        composeRule.waitForIdle()
        composeRule.onNodeWithText(chooseDark).assertIsSelected()
        assertEquals(
            "the record holds what the recreated app is showing",
            ReadingPreferences(appearance = SlipboxAppearance.Dark),
            stored(),
        )
    }

    @Composable
    private fun Hosted() {
        CompositionLocalProvider(LocalContext provides host) {
            platformScale = rememberPlatformMotionScale()
            SlipboxApp()
        }
    }

    private fun show() {
        composeRule.setContent { Hosted() }
        composeRule.waitForIdle()
    }

    private fun open() {
        composeRule.onNodeWithText(forward).performClick()
        composeRule.waitForIdle()
        composeRule.onNodeWithText(about).assertIsDisplayed()
    }

    private fun showing(text: String): Boolean =
        composeRule.onAllNodesWithText(text).fetchSemanticsNodes().isNotEmpty()

    private fun revealed(): Boolean =
        composeRule
            .onAllNodes(SemanticsMatcher.expectValue(SemanticsProperties.PaneTitle, paneTitle))
            .fetchSemanticsNodes()
            .isNotEmpty()

    private fun goneAtOnce(what: String, text: String) {
        val gone = composeRule.mainClock.frames { !showing(text) }
        assertTrue("$what: after $gone frame(s)", gone <= COMPOSED)
    }

    private fun sheetTop(): Float =
        composeRule
            .onNode(SemanticsMatcher.expectValue(SemanticsProperties.PaneTitle, paneTitle))
            .bounds()
            .top

    private fun scale(value: Float) {
        val written =
            Settings.Global.putFloat(
                context.contentResolver,
                Settings.Global.ANIMATOR_DURATION_SCALE,
                value,
            )
        check(written) { "the synthetic device refused the animator scale" }
    }

    private fun recorded(preferences: ReadingPreferences) {
        assertNull(
            "the suite's own record was written",
            ReadingPreferencesFile(record).write(preferences),
        )
    }

    private fun stored(): ReadingPreferences? =
        (ReadingPreferencesFile(record).read() as? ReadingPreferencesRecord.Stored)?.preferences

    private companion object {
        const val TIMEOUT = 5_000L

        const val OWNED = "root-motion"

        const val EDGE = 1f

        /** Allow framework scheduling, but less than a full crossfade. */
        const val COMPOSED = 5

        val HALFWAY = SlipboxTokens.Motion.CROSSFADE_MS / 2L

        /** Past one crossfade, short of double scaling. */
        val ONWARD = SlipboxTokens.Motion.CROSSFADE_MS / 2L + 48L

        val RESTED = 2L * SlipboxTokens.Motion.COLUMN_MS

        val STEP = SlipboxTokens.Motion.COLUMN_MS / 4L
    }
}
