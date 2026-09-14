/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.navigation

import androidx.activity.ComponentActivity
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.StateRestorationTester
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.espresso.Espresso
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.ui.MemoryStore
import io.github.b_vitamins.slipbox.ui.Specimen
import io.github.b_vitamins.slipbox.ui.settings.ReadingSettings
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotSame
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class NavigationLifecycleTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val appearance = context.getString(R.string.action_appearance)
    private val paneTitle = context.getString(R.string.appearance_title)

    private val settings = ReadingSettings(MemoryStore())
    private val restoration = StateRestorationTester(composeRule)
    private var owner: SlipboxBackStack? = null

    @Test
    fun theHistoryAndTheReadingPositionSurviveRecreation() {
        show()
        composeRule.onNodeWithText(Synthetic.SEARCH).performClick()
        open()
        composeRule.runOnIdle {
            assertTrue(history().open(Synthetic.note(mark = Synthetic.MARK)))
            assertTrue(history().open(Synthetic.glossary()))
        }
        composeRule.onNodeWithText(Synthetic.TERM).assertIsDisplayed()

        restoration.emulateSavedInstanceStateRestore()
        composeRule.waitForIdle()

        composeRule.onNodeWithText(Synthetic.TERM).assertIsDisplayed()
        val restored = history().entries
        assertEquals(3, restored.size)
        assertEquals(SlipboxRoute.Library(Synthetic.SEARCHED), restored.first())
        assertEquals(Synthetic.note(mark = Synthetic.MARK), restored[1])

        Espresso.pressBack()
        composeRule.waitForIdle()
        composeRule.onNodeWithText(Specimen.TITLE).assertIsDisplayed()
        Espresso.pressBack()
        composeRule.waitForIdle()
        composeRule.onNodeWithText(Synthetic.SEARCHED).assertIsDisplayed()
    }

    @Test
    fun anEntryKeepsItsPresentationWhenOnlyItsPositionChanges() {
        show()
        open()
        composeRule.onNodeWithText(appearance).performClick()
        composeRule.waitForIdle()
        assertTrue("the reader's own reveal is up", composeRule.revealing(paneTitle))

        composeRule.runOnIdle { assertTrue(history().open(Synthetic.note(mark = Synthetic.MARK))) }
        composeRule.waitForIdle()

        assertEquals("reading the same note again is one entry", 2, history().entries.size)
        assertTrue("and it kept what it was showing", composeRule.revealing(paneTitle))

        val elsewhere = Synthetic.note(source = Synthetic.BETA)
        composeRule.runOnIdle { assertTrue(history().open(elsewhere)) }
        composeRule.waitForIdle()

        assertEquals(3, history().entries.size)
        assertFalse("which brought nothing of its own up", composeRule.revealing(paneTitle))
    }

    @Test
    fun aRequestFromAPresentationThatHasGoneDoesNotReachTheOwnerThatReplacedIt() {
        show()
        open()
        val gone = history()

        restoration.emulateSavedInstanceStateRestore()
        composeRule.waitForIdle()

        val live = history()
        assertNotSame("recreation made another owner", gone, live)
        composeRule.runOnIdle {
            assertFalse(gone.open(Synthetic.glossary()))
            assertFalse(gone.back())
        }
        composeRule.waitForIdle()

        assertEquals(2, live.entries.size)
        composeRule.onNodeWithText(Specimen.TITLE).assertIsDisplayed()
    }

    private fun show() {
        restoration.setContent { SyntheticHost(settings, onOwner = { owner = it }) }
        composeRule.waitForIdle()
    }

    private fun open() {
        composeRule.onNodeWithText(Synthetic.OPEN).performClick()
        composeRule.waitForIdle()
        composeRule.onNodeWithText(Specimen.TITLE).assertIsDisplayed()
    }

    private fun history(): SlipboxBackStack = checkNotNull(owner) { "no destination presented" }
}
