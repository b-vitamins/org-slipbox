/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import android.os.Build
import androidx.activity.ComponentActivity
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotSelected
import androidx.compose.ui.test.assertIsOff
import androidx.compose.ui.test.assertIsOn
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.StateRestorationTester
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.test.espresso.Espresso
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.SdkSuppress
import androidx.test.platform.app.InstrumentationRegistry
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.ui.settings.ReadingPreferenceFault
import io.github.b_vitamins.slipbox.ui.settings.ReadingPreferences
import io.github.b_vitamins.slipbox.ui.settings.ReadingPreferencesFile
import io.github.b_vitamins.slipbox.ui.settings.ReadingPreferencesRecord
import io.github.b_vitamins.slipbox.ui.settings.ReadingPreferencesStore
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

/** The one reveal a reader changes the surface from, and what it leaves behind. */
@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
@SdkSuppress(minSdkVersion = Build.VERSION_CODES.O)
class AppearanceTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    private val context = InstrumentationRegistry.getInstrumentation().targetContext

    /** This suite's own record, beside the app's own rather than in place of it. */
    private val record = File(context.noBackupFilesDir, "design-reading-preferences")

    private val control = context.getString(R.string.action_appearance)
    private val paneTitle = context.getString(R.string.appearance_title)
    private val followSystem = context.getString(R.string.appearance_system)
    private val chooseLight = context.getString(R.string.appearance_light)
    private val chooseDark = context.getString(R.string.appearance_dark)
    private val reduceMotion = context.getString(R.string.appearance_reduce_motion)
    private val unwritable = context.getString(R.string.appearance_unwritable)
    private val unreadable = context.getString(R.string.appearance_unreadable)

    private var shown by mutableStateOf(LIGHT_PLATFORM)
    private lateinit var settings: ReadingSettings

    @Before
    @After
    fun discardOwnRecord() {
        record.delete()
        File(record.parentFile, "${record.name}.pending").delete()
    }

    @Test
    fun theControlRevealsTheChoicesAndEachCarriesItsRole() {
        show()
        composeRule.onNodeWithText(followSystem).assertDoesNotExist()
        reveal()
        composeRule
            .onNode(SemanticsMatcher.expectValue(SemanticsProperties.PaneTitle, paneTitle))
            .assertExists()
        composeRule.onNodeWithText(followSystem).assertRole(Role.RadioButton).assertIsSelected()
        composeRule.onNodeWithText(chooseLight).assertRole(Role.RadioButton).assertIsNotSelected()
        composeRule.onNodeWithText(chooseDark).assertRole(Role.RadioButton).assertIsNotSelected()
        composeRule.onNodeWithText(reduceMotion).assertRole(Role.Switch).assertIsOff()
    }

    @Test
    fun choosingASchemeRepaintsTheSurfaceItWasChosenOn() {
        show()
        val before = surfaceColour()
        assertEquals("the platform's own scheme", hex(paper(dark = false)), hex(before))
        choose(chooseDark)
        val after = surfaceColour()
        assertEquals("the chosen scheme", hex(paper(dark = true)), hex(after))
        assertEquals(SlipboxAppearance.Dark, settings.preferences.appearance)
        Evidence.record(
            "appearance-choice",
            Record()
                .colour("platformPaper", before)
                .colour("chosenPaper", after)
                .text("chosen", settings.preferences.appearance.name),
        )
    }

    @Test
    fun theSystemChoiceFollowsThePlatformAndAChosenSchemeDoesNot() {
        show()
        assertEquals(hex(paper(dark = false)), hex(surfaceColour()))
        platform(DARK_PLATFORM)
        assertEquals("the system was followed", hex(paper(dark = true)), hex(surfaceColour()))
        choose(chooseLight)
        assertEquals("the choice was taken", hex(paper(dark = false)), hex(surfaceColour()))
        platform(LIGHT_PLATFORM)
        platform(DARK_PLATFORM)
        assertEquals("the choice still stands", hex(paper(dark = false)), hex(surfaceColour()))
    }

    @Test
    fun backRetiresTheRevealAndLeavesTheSurfaceStanding() {
        show()
        reveal()
        Espresso.pressBack()
        composeRule.waitForIdle()
        composeRule.onNodeWithText(followSystem).assertDoesNotExist()
        composeRule.onNodeWithText(Specimen.TITLE).assertIsDisplayed()
        composeRule.onNodeWithText(control).assertIsDisplayed()
    }

    @Test
    fun theRevealSurvivesTheRecreationOfTheSurfaceUnderIt() {
        val tester = StateRestorationTester(composeRule)
        settings = ReadingSettings(MemoryStore())
        tester.setContent {
            shown.Content(appearance = settings.preferences.appearance) {
                SpecimenNote(settings = settings)
            }
        }
        reveal()
        composeRule.onNodeWithText(followSystem).assertIsDisplayed()
        tester.emulateSavedInstanceStateRestore()
        composeRule.waitForIdle()
        composeRule.onNodeWithText(followSystem).assertIsDisplayed()
    }

    @Test
    fun aChoiceOutlivesTheSettingsThatRecordedIt() {
        show(ReadingPreferencesFile(record))
        choose(chooseDark)
        reveal()
        composeRule.onNodeWithText(reduceMotion).performClick()
        composeRule.waitForIdle()
        composeRule.onNodeWithText(reduceMotion).assertIsOn()
        assertTrue("the record was written", record.exists())

        val later = ReadingSettings(ReadingPreferencesFile(record))
        assertEquals(SlipboxAppearance.Dark, later.preferences.appearance)
        assertTrue(later.preferences.reduceMotion)
        assertTrue("the record was usable", later.stored)
        assertNull(later.fault)
    }

    @Test
    fun aChoiceThatCouldNotBeRecordedStillAppliesAndSaysSo() {
        show(MemoryStore(fault = ReadingPreferenceFault.Unwritable))
        choose(chooseDark)
        reveal()
        composeRule.onNodeWithText(chooseDark).assertIsSelected()
        composeRule.onNodeWithText(unwritable).performScrollTo().assertIsDisplayed()
        assertEquals(SlipboxAppearance.Dark, settings.preferences.appearance)
        assertFalse("nothing was recorded", settings.stored)
    }

    @Test
    fun anUnusableRecordLeavesTheDefaultsShowingAndSaysSo() {
        val unusable = ReadingPreferencesRecord.Unusable(ReadingPreferenceFault.Malformed)
        show(MemoryStore(record = unusable))
        reveal()
        composeRule.onNodeWithText(followSystem).assertIsSelected()
        composeRule.onNodeWithText(reduceMotion).assertIsOff()
        composeRule.onNodeWithText(unreadable).performScrollTo().assertIsDisplayed()
        assertEquals(ReadingPreferences(), settings.preferences)
        assertFalse(settings.stored)
    }

    private fun show(store: ReadingPreferencesStore = MemoryStore()) {
        settings = ReadingSettings(store)
        composeRule.setContent {
            shown.Content(appearance = settings.preferences.appearance) {
                SpecimenNote(settings = settings)
            }
        }
        composeRule.waitForIdle()
    }

    private fun reveal() {
        composeRule.onNodeWithText(control).performClick()
        composeRule.waitForIdle()
    }

    /** Opens the reveal, takes one choice and retires it. */
    private fun choose(label: String) {
        reveal()
        composeRule.onNodeWithText(label).performClick()
        composeRule.waitForIdle()
        Espresso.pressBack()
        composeRule.waitForIdle()
    }

    private fun platform(viewport: VisualCase) {
        composeRule.runOnIdle { shown = viewport }
        composeRule.waitForIdle()
    }

    private fun surfaceColour(): Color =
        composeRule.onNodeWithTag(Specimen.SURFACE).captureToImage().pixels()[2, 2]

    private fun SemanticsNodeInteraction.assertRole(role: Role): SemanticsNodeInteraction =
        assert(SemanticsMatcher.expectValue(SemanticsProperties.Role, role))

    private companion object {
        val LIGHT_PLATFORM = VISUAL_CASES.first { !it.dark && it.fontScale == 1f }
        val DARK_PLATFORM = VISUAL_CASES.first { it.dark && it.fontScale == 1f }
    }
}
