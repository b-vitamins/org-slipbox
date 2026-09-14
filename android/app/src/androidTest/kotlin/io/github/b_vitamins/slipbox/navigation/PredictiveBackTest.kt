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
import androidx.navigationevent.DirectNavigationEventInput
import androidx.navigationevent.NavigationEvent
import androidx.test.ext.junit.runners.AndroidJUnit4
import io.github.b_vitamins.slipbox.ui.Evidence
import io.github.b_vitamins.slipbox.ui.MemoryStore
import io.github.b_vitamins.slipbox.ui.Record
import io.github.b_vitamins.slipbox.ui.Specimen
import io.github.b_vitamins.slipbox.ui.settings.ReadingSettings
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class PredictiveBackTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    private val settings = ReadingSettings(MemoryStore())
    private val gesture = DirectNavigationEventInput()
    private var owner: SlipboxBackStack? = null
    private var attached = false

    @After
    fun stopDrivingTheHostsDispatcher() {
        if (!attached) return
        composeRule.runOnUiThread {
            composeRule.activity.navigationEventDispatcher.removeInput(gesture)
        }
        attached = false
    }

    @Test
    fun aCompletedGestureLeavesTheDestinationItRevealed() {
        show()
        open()
        drag(START, ONWARD)
        assertTrue("the destination beneath is revealed as the gesture runs", revealed())

        composeRule.runOnUiThread { gesture.backCompleted() }
        composeRule.waitForIdle()

        assertEquals(listOf(SlipboxRoute.Start), history().entries)
        composeRule.onNodeWithText(Synthetic.LIBRARY).assertIsDisplayed()
        Evidence.record(
            "predictive-back-completed",
            Record()
                .size("progress", ONWARD)
                .count("entriesAfter", history().entries.size)
                .flag("revealedDuringGesture", true),
        )
    }

    @Test
    fun aCancelledGestureLeavesTheHistoryWhereItWas() {
        show()
        open()
        drag(START, ONWARD)
        assertTrue("the destination beneath is revealed as the gesture runs", revealed())

        composeRule.runOnUiThread { gesture.backCancelled() }
        composeRule.waitForIdle()

        assertEquals(2, history().entries.size)
        assertEquals(Synthetic.note(), history().current)
        composeRule.onNodeWithText(Specimen.TITLE).assertIsDisplayed()
        Evidence.record(
            "predictive-back-cancelled",
            Record()
                .size("progress", ONWARD)
                .count("entriesAfter", history().entries.size)
                .flag("presentedIsTheNoteRead", history().current == Synthetic.note()),
        )
    }

    private fun show() {
        composeRule.setContent { SyntheticHost(settings, onOwner = { owner = it }) }
        composeRule.waitForIdle()
        composeRule.runOnUiThread {
            composeRule.activity.navigationEventDispatcher.addInput(gesture)
        }
        attached = true
    }

    private fun open() {
        composeRule.onNodeWithText(Synthetic.OPEN).performClick()
        composeRule.waitForIdle()
        composeRule.onNodeWithText(Specimen.TITLE).assertIsDisplayed()
        assertEquals(2, history().entries.size)
    }

    private fun drag(vararg progress: Float) {
        composeRule.runOnUiThread {
            gesture.backStarted(NavigationEvent(NavigationEvent.EDGE_LEFT, progress.first()))
            for (step in progress.drop(1)) {
                gesture.backProgressed(NavigationEvent(NavigationEvent.EDGE_LEFT, step))
            }
        }
        composeRule.waitForIdle()
    }

    private fun revealed(): Boolean = composeRule.showing(Synthetic.LIBRARY)

    private fun history(): SlipboxBackStack = checkNotNull(owner) { "no destination presented" }

    private companion object {
        const val START = 0.1f
        const val ONWARD = 0.6f
    }
}
