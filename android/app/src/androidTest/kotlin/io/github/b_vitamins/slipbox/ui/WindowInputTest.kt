/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.OutlinedTextField
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.DeviceConfigurationOverride
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.FontScale
import androidx.compose.ui.test.WindowInsets
import androidx.compose.ui.test.WindowSize
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.then
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import androidx.core.graphics.Insets
import androidx.core.view.WindowInsetsCompat
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.sources.SourceLibraryPhase
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTheme
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
class WindowInputTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    private val context = InstrumentationRegistry.getInstrumentation().targetContext

    @Test
    fun systemBarsAndImeBoundFocusedLargeTypeWithoutShrinkingTargets() {
        lateinit var density: Density
        val insets =
            WindowInsetsCompat.Builder()
                .setInsets(WindowInsetsCompat.Type.statusBars(), Insets.of(LEFT, TOP, RIGHT, 0))
                .setInsets(WindowInsetsCompat.Type.navigationBars(), Insets.of(LEFT, 0, RIGHT, 0))
                .setInsets(WindowInsetsCompat.Type.ime(), Insets.of(0, 0, 0, IME))
                .setVisible(WindowInsetsCompat.Type.statusBars(), true)
                .setVisible(WindowInsetsCompat.Type.navigationBars(), true)
                .setVisible(WindowInsetsCompat.Type.ime(), true)
                .build()
        composeRule.setContent {
            DeviceConfigurationOverride(
                DeviceConfigurationOverride.WindowSize(DpSize(320.dp, 640.dp)) then
                    DeviceConfigurationOverride.FontScale(2f) then
                    DeviceConfigurationOverride.WindowInsets(insets),
            ) {
                SlipboxTheme {
                    density = LocalDensity.current
                    val field = remember { FocusRequester() }
                    ReadingSurface(
                        title = "Insets",
                        modifier = Modifier.testTag(SURFACE),
                        scrollable = false,
                        leading = {
                            TextControl(
                                label = "Leading",
                                onClick = {},
                                modifier = Modifier.testTag(LEADING),
                            )
                        },
                        trailing = {
                            TextControl(
                                label = "Done",
                                onClick = {},
                                modifier = Modifier.testTag(TRAILING),
                            )
                        },
                    ) {
                        Column(modifier = Modifier.fillMaxSize()) {
                            Spacer(Modifier.weight(1f))
                            OutlinedTextField(
                                value = "Keyboard",
                                onValueChange = {},
                                modifier =
                                    Modifier
                                        .fillMaxWidth()
                                        .focusRequester(field)
                                        .testTag(FIELD),
                            )
                        }
                    }
                    LaunchedEffect(Unit) { field.requestFocus() }
                }
            }
        }
        composeRule.waitForIdle()

        val surface = composeRule.onNodeWithTag(SURFACE).getUnclippedBoundsInRoot()
        val leading = composeRule.onNodeWithTag(LEADING).getUnclippedBoundsInRoot()
        val trailing = composeRule.onNodeWithTag(TRAILING).getUnclippedBoundsInRoot()
        val field =
            composeRule
                .onNodeWithTag(FIELD)
                .assertIsFocused()
                .getUnclippedBoundsInRoot()
        val leftInset = with(density) { LEFT.toDp() }
        val topInset = with(density) { TOP.toDp() }
        val rightInset = with(density) { RIGHT.toDp() }
        val imeInset = with(density) { IME.toDp() }

        assertTrue("status inset is clear", leading.top >= surface.top + topInset)
        assertTrue("left system inset is clear", leading.left >= surface.left + leftInset)
        assertTrue("right system inset is clear", trailing.right <= surface.right - rightInset)
        assertTrue("focused input stays above the IME", field.bottom <= surface.bottom - imeInset)
        assertTrue("leading target remains tappable", leading.bottom - leading.top >= 48.dp)
        assertTrue("trailing target remains tappable", trailing.bottom - trailing.top >= 48.dp)
    }

    @Test
    fun compactLargeTypeKeepsEveryLibraryActionClearAndTappable() {
        composeRule.setContent {
            DeviceConfigurationOverride(
                DeviceConfigurationOverride.WindowSize(DpSize(320.dp, 640.dp)) then
                    DeviceConfigurationOverride.FontScale(2f),
            ) {
                SlipboxTheme {
                    LibraryScreen(
                        onOpenAbout = {},
                        phase = SourceLibraryPhase.Empty(0),
                        hasSources = true,
                    )
                }
            }
        }

        val title =
            composeRule
                .onNodeWithText(context.getString(R.string.app_name))
                .assertIsDisplayed()
                .getUnclippedBoundsInRoot()
        val actions =
            listOf(R.string.action_sources, R.string.action_add_source, R.string.action_about)
                .map { resource ->
                    composeRule
                        .onNodeWithContentDescription(context.getString(resource))
                        .assertIsDisplayed()
                        .getUnclippedBoundsInRoot()
                }
        val target = 48.dp

        assertTrue("the title clears the actions", title.right <= actions.first().left)
        actions.forEach { action ->
            assertTrue("action width remains tappable", action.right - action.left >= target)
            assertTrue("action height remains tappable", action.bottom - action.top >= target)
        }
        actions.zipWithNext().forEach { (left, right) ->
            assertTrue("library actions do not overlap", left.right <= right.left)
        }
    }

    private companion object {
        const val LEFT = 24
        const val TOP = 48
        const val RIGHT = 36
        const val IME = 240
        const val SURFACE = "window-input-surface"
        const val LEADING = "window-input-leading"
        const val TRAILING = "window-input-trailing"
        const val FIELD = "window-input-field"
    }
}
