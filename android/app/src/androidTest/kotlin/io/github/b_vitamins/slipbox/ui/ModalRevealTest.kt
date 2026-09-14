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
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusDirection
import androidx.compose.ui.focus.FocusManager
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.InputMode
import androidx.compose.ui.input.InputModeManager
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalInputModeManager
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertHasClickAction
import androidx.compose.ui.test.assertHasNoClickAction
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsNotFocused
import androidx.compose.ui.test.assertIsNotSelected
import androidx.compose.ui.test.assertIsOff
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.click
import androidx.compose.ui.test.isFocused
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.requestFocus
import androidx.test.espresso.Espresso
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.SdkSuppress
import androidx.test.platform.app.InstrumentationRegistry
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.ui.settings.ReadingSettings
import io.github.b_vitamins.slipbox.ui.theme.SlipboxMotion
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
@SdkSuppress(minSdkVersion = Build.VERSION_CODES.O)
class ModalRevealTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    private val context = InstrumentationRegistry.getInstrumentation().targetContext

    private val control = context.getString(R.string.action_appearance)
    private val back = context.getString(R.string.action_back)
    private val dismissal = context.getString(R.string.action_dismiss)
    private val paneTitle = context.getString(R.string.appearance_title)
    private val followSystem = context.getString(R.string.appearance_system)
    private val chooseDark = context.getString(R.string.appearance_dark)
    private val reduceMotion = context.getString(R.string.appearance_reduce_motion)

    private val pane = SemanticsMatcher.expectValue(SemanticsProperties.PaneTitle, paneTitle)

    private val scrim =
        SemanticsMatcher("a dismissal") { node ->
            node.config.getOrNull(SemanticsActions.OnClick)?.label == dismissal
        }

    private lateinit var settings: ReadingSettings
    private lateinit var focus: FocusManager
    private lateinit var inputMode: InputModeManager
    private var backs = 0
    private var links = 0

    @Test
    fun theSurfaceUnderARevealIsWithheldFromAccessibility() {
        show()
        composeRule.onAllNodes(WITHHELD, useUnmergedTree = true).assertCountEquals(0)
        reveal()
        composeRule.onAllNodes(WITHHELD, useUnmergedTree = true).assertCountEquals(1)
        composeRule
            .onNode(pane, useUnmergedTree = true)
            .assert(SemanticsMatcher.keyNotDefined(SemanticsProperties.HideFromAccessibility))
        Espresso.pressBack()
        composeRule.waitForIdle()
        composeRule.onAllNodes(WITHHELD, useUnmergedTree = true).assertCountEquals(0)
        Evidence.record(
            "modal-ownership",
            Record()
                .text("pane", paneTitle)
                .count("withheldWhileRevealed", 1)
                .count("withheldWhenRetired", 0)
                .count("backgroundActions", backs + links),
        )
    }

    @Test
    fun theControlsUnderARevealCarryNoActionToFire() {
        show()
        for (node in controls()) node.assertIsEnabled().assertHasClickAction()
        reveal()
        for (node in controls()) {
            node.assertIsNotEnabled()
                .assertHasNoClickAction()
                .assert(SemanticsMatcher.keyNotDefined(SemanticsActions.RequestFocus))
                .assert(SemanticsMatcher.keyNotDefined(SemanticsProperties.Focused))
        }
        composeRule.onNodeWithText(followSystem).assertIsEnabled().assertHasClickAction()
        assertEquals("nothing under the reveal acted", 0, backs + links)
        Espresso.pressBack()
        composeRule.waitForIdle()
        for (node in controls()) node.assertIsEnabled().assertHasClickAction()
        assertEquals("retiring the reveal acted on nothing either", 0, backs + links)
    }

    @Test
    fun aTapWhereACoveredControlSitsRetiresTheRevealInstead() {
        show()
        reveal()
        composeRule.onNodeWithContentDescription(back).performTouchInput { click() }
        composeRule.waitForIdle()
        assertEquals("the covered control did not act", 0, backs)
        composeRule.onNodeWithText(followSystem).assertDoesNotExist()
        composeRule.onNodeWithText(Specimen.TITLE).assertIsDisplayed()
    }

    @Test
    fun keyboardFocusStaysInTheRevealAndCannotReachTheSurfaceUnderIt() {
        show()
        reveal()
        takeKeyboard()
        composeRule.onNodeWithText(followSystem).requestFocus().assertIsFocused()
        val reveal = composeRule.onNode(pane, useUnmergedTree = true).bounds()
        repeat(MOVES) { move ->
            composeRule.runOnUiThread { focus.moveFocus(FocusDirection.Next) }
            composeRule.waitForIdle()
            val focused = composeRule.onNode(isFocused()).bounds()
            assertTrue(
                "move ${move + 1} left the reveal: $focused is not within $reveal",
                focused.top >= reveal.top - EDGE && focused.bottom <= reveal.bottom + EDGE,
            )
        }
        for (node in controls()) {
            node.assert(SemanticsMatcher.keyNotDefined(SemanticsProperties.Focused))
        }
    }

    @Test
    fun retiringTheRevealReturnsFocusToTheControlItWasOpenedFrom() {
        show()
        reveal()
        takeKeyboard()
        composeRule.onNodeWithText(followSystem).requestFocus().assertIsFocused()
        Espresso.pressBack()
        composeRule.waitForIdle()
        composeRule.onNodeWithText(control).assertIsFocused()

        reveal()
        composeRule.onNodeWithText(reduceMotion).requestFocus().assertIsFocused()
        composeRule.onNode(scrim).performTouchInput { click(Offset(EDGE, EDGE)) }
        composeRule.waitForIdle()
        composeRule.onNodeWithText(control).assertIsFocused()
        assertEquals("neither retirement left the surface", 0, backs)
    }

    @Test
    fun aRevealThatNeverTookFocusDoesNotMoveIt() {
        show()
        reveal()
        takeKeyboard()
        composeRule.onAllNodes(isFocused()).assertCountEquals(0)
        Espresso.pressBack()
        composeRule.waitForIdle()
        composeRule.onAllNodes(isFocused()).assertCountEquals(0)
        composeRule.onNodeWithText(control).assertIsNotFocused()
    }

    @Test
    fun aStaleRetirementOrASecondRevealTakesNothingBack() {
        val retire = hosted(SlipboxMotion(reduceMotion = true), spare = true)
        composeRule.onAllNodes(pane, useUnmergedTree = true).assertCountEquals(1)
        composeRule.onAllNodesWithText(followSystem).assertCountEquals(1)

        composeRule.onNodeWithText(followSystem).requestFocus().assertIsFocused()
        retire()
        composeRule.waitForIdle()
        composeRule.onNodeWithText(control).assertIsFocused()

        composeRule.onNodeWithContentDescription(back).requestFocus().assertIsFocused()
        retire()
        retire()
        composeRule.waitForIdle()
        composeRule.onNodeWithContentDescription(back).assertIsFocused()
        composeRule.onNodeWithText(control).assertIsNotFocused()
        assertEquals("no retirement acted on the surface", 0, backs)
    }

    @Test
    fun aRevealStillLeavingHasAlreadyGivenBackWhatItHeld() {
        val retire = hosted(SlipboxMotion())
        composeRule.onNodeWithText(followSystem).requestFocus().assertIsFocused()

        composeRule.mainClock.autoAdvance = false
        retire()
        val given = composeRule.mainClock.frames { focused(control) }
        assertTrue("the control it was opened from has focus: after $given frame(s)", given <= SOON)
        assertTrue("while the reveal is still leaving", revealed())

        composeRule.mainClock.autoAdvance = true
        composeRule.waitForIdle()
        composeRule.onNodeWithText(followSystem).assertDoesNotExist()
        composeRule.onNodeWithText(control).assertIsFocused()
        assertEquals("and the retirement acted on nothing", 0, backs)
    }

    @Test
    fun namesRolesAndSelectionSurviveTheRevealBeingReopened() {
        show()
        reveal()
        composeRule.onNodeWithText(followSystem).assertRole(Role.RadioButton).assertIsSelected()
        composeRule.onNodeWithText(chooseDark).assertRole(Role.RadioButton).assertIsNotSelected()
        composeRule.onNodeWithText(reduceMotion).assertRole(Role.Switch).assertIsOff()
        composeRule.onNodeWithText(chooseDark).performClick()
        composeRule.waitForIdle()
        Espresso.pressBack()
        composeRule.waitForIdle()
        composeRule.onNodeWithText(chooseDark).assertDoesNotExist()
        reveal()
        composeRule.onNode(pane, useUnmergedTree = true).assertExists()
        composeRule.onNodeWithText(chooseDark).assertRole(Role.RadioButton).assertIsSelected()
        composeRule.onNodeWithText(followSystem).assertRole(Role.RadioButton).assertIsNotSelected()
        composeRule.onNodeWithText(reduceMotion).assertRole(Role.Switch).assertIsOff()
    }

    private fun show() {
        settings = ReadingSettings(MemoryStore())
        composeRule.setContent {
            focus = LocalFocusManager.current
            inputMode = LocalInputModeManager.current
            LIGHT.Content(appearance = settings.preferences.appearance) {
                SpecimenNote(settings = settings, onBack = { backs++ }, onLink = { links++ })
            }
        }
        composeRule.waitForIdle()
    }

    private fun reveal() {
        composeRule.onNodeWithText(control).performClick()
        composeRule.waitForIdle()
    }

    private fun hosted(motion: SlipboxMotion, spare: Boolean = false): () -> Unit {
        var open by mutableStateOf(true)
        var retire: () -> Unit = {}
        settings = ReadingSettings(MemoryStore())
        composeRule.setContent {
            focus = LocalFocusManager.current
            inputMode = LocalInputModeManager.current
            LIGHT.Content(appearance = settings.preferences.appearance) {
                val opened = remember { FocusRequester() }
                retire = { open = false }
                ReadingSurface(
                    title = Specimen.TITLE,
                    obscured = open,
                    leading = {
                        IconControl(
                            icon = painterResource(R.drawable.ic_back),
                            label = back,
                            onClick = { backs++ },
                        )
                    },
                    trailing = {
                        TextControl(
                            label = control,
                            onClick = { open = true },
                            modifier = Modifier.focusRequester(opened),
                        )
                    },
                    overlay = {
                        AppearanceSheet(
                            visible = open,
                            settings = settings,
                            motion = motion,
                            onDismiss = { open = false },
                            restoreFocusTo = opened,
                        )
                        if (spare) {
                            // A second reveal, never shown, told to return to that control.
                            AppearanceSheet(
                                visible = false,
                                settings = settings,
                                motion = motion,
                                onDismiss = {},
                                restoreFocusTo = opened,
                            )
                        }
                    },
                    body = {},
                )
            }
        }
        composeRule.waitForIdle()
        takeKeyboard()
        return { composeRule.runOnUiThread { retire() } }
    }

    private fun revealed(): Boolean =
        composeRule.onAllNodes(pane, useUnmergedTree = true).fetchSemanticsNodes().isNotEmpty()

    private fun focused(text: String): Boolean =
        composeRule.onAllNodesWithText(text).fetchSemanticsNodes().any { node ->
            node.config.getOrNull(SemanticsProperties.Focused) == true
        }

    private fun controls(): List<SemanticsNodeInteraction> =
        listOf(
            composeRule.onNodeWithText(control),
            composeRule.onNodeWithContentDescription(back),
        )

    /** Focus moves only once the window holds it and the input mode is not touch. */
    private fun takeKeyboard() {
        composeRule.waitUntil(TIMEOUT) { composeRule.activity.hasWindowFocus() }
        assertTrue(
            "the surface leaves touch input",
            composeRule.runOnUiThread { inputMode.requestInputMode(InputMode.Keyboard) },
        )
    }

    private fun SemanticsNodeInteraction.assertRole(role: Role): SemanticsNodeInteraction =
        assert(SemanticsMatcher.expectValue(SemanticsProperties.Role, role))

    private companion object {
        const val TIMEOUT = 5_000L

    /** Traverse past the last row. */
        const val MOVES = 6

        const val EDGE = 1f

        /** Allow focus restoration, but less than a full transition. */
        const val SOON = 4

        val LIGHT = VISUAL_CASES.first { !it.dark && it.fontScale == 1f }
    }
}
