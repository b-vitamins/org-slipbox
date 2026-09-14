/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.navigation

import androidx.activity.ComponentActivity
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.espresso.Espresso
import androidx.test.espresso.NoActivityResumedException
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.ui.MemoryStore
import io.github.b_vitamins.slipbox.ui.Specimen
import io.github.b_vitamins.slipbox.ui.settings.ReadingSettings
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class NavigationBackTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val appearance = context.getString(R.string.action_appearance)
    private val paneTitle = context.getString(R.string.appearance_title)

    private val settings = ReadingSettings(MemoryStore())
    private var owner: SlipboxBackStack? = null

    @Test
    fun systemBackLeavesTheDestinationAndKeepsTheSearchBeneathIt() {
        show()
        composeRule.onNodeWithText(Synthetic.SEARCH).performClick()
        composeRule.waitForIdle()
        composeRule.onNodeWithText(Synthetic.SEARCHED).assertIsDisplayed()
        open()

        Espresso.pressBack()
        composeRule.waitForIdle()

        composeRule.onNodeWithText(Synthetic.SEARCHED).assertIsDisplayed()
        assertEquals(listOf(SlipboxRoute.Library(Synthetic.SEARCHED)), history().entries)
    }

    @Test
    fun theUppermostSheetIsDismissedBeforeTheDestinationBeneathIt() {
        show()
        open()
        composeRule.onNodeWithText(appearance).performClick()
        composeRule.waitForIdle()
        assertTrue("the sheet the test host provides is up", composeRule.revealing(paneTitle))

        Espresso.pressBack()
        composeRule.waitForIdle()

        assertFalse("the sheet went first", composeRule.revealing(paneTitle))
        composeRule.onNodeWithText(Specimen.TITLE).assertIsDisplayed()
        assertEquals("and the destination stayed where it was", 2, history().entries.size)

        Espresso.pressBack()
        composeRule.waitForIdle()

        assertEquals(listOf(SlipboxRoute.Start), history().entries)
        composeRule.onNodeWithText(Synthetic.LIBRARY).assertIsDisplayed()
    }

    @Test
    fun theRootBelongsToThePlatform() {
        show()
        composeRule.onNodeWithText(Synthetic.LIBRARY).assertIsDisplayed()
        assertEquals(1, history().entries.size)

        assertThrows(NoActivityResumedException::class.java) { Espresso.pressBack() }
    }

    private fun show() {
        composeRule.setContent { SyntheticHost(settings, onOwner = { owner = it }) }
        composeRule.waitForIdle()
    }

    private fun open() {
        composeRule.onNodeWithText(Synthetic.OPEN).performClick()
        composeRule.waitForIdle()
        composeRule.onNodeWithText(Specimen.TITLE).assertIsDisplayed()
        assertEquals(2, history().entries.size)
    }

    private fun history(): SlipboxBackStack = checkNotNull(owner) { "no destination presented" }
}
