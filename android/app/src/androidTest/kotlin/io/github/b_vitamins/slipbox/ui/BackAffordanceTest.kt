/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertHeightIsAtLeast
import androidx.compose.ui.test.assertWidthIsAtLeast
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.unit.dp
import androidx.test.ext.junit.runners.AndroidJUnit4
import io.github.b_vitamins.slipbox.SlipboxApp
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTheme
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class BackAffordanceTest {

    @get:Rule val composeRule = createComposeRule()

    @Test
    fun theBackControlIsAMarkWithAnAccessibleNameAndAReachableTarget() {
        var popped = false
        composeRule.setContent { SlipboxTheme { AboutScreen(onBack = { popped = true }) } }

        composeRule.onNodeWithText(BACK).assertDoesNotExist()
        composeRule
            .onNodeWithContentDescription(BACK, useUnmergedTree = true)
            .assert(hasClickAction())
            .assertWidthIsAtLeast(TOUCH_TARGET)
            .assertHeightIsAtLeast(TOUCH_TARGET)
            .performClick()

        assertTrue(popped)
    }

    @Test
    fun repeatedForwardCallbacksNeedOnlyOneBackToReachTheRoot() {
        composeRule.setContent { SlipboxApp() }
        val openAbout =
            composeRule.onNodeWithText("About")
                .fetchSemanticsNode().config[SemanticsActions.OnClick].action!!

        composeRule.runOnIdle {
            openAbout()
            openAbout()
        }
        composeRule.onNodeWithContentDescription(BACK, useUnmergedTree = true).performClick()
        composeRule.onNodeWithText("Nothing to read yet.").assertExists()
        composeRule.onNodeWithContentDescription(BACK).assertDoesNotExist()
    }

    private companion object {
        const val BACK = "Back"
        val TOUCH_TARGET = 48.dp
    }
}
