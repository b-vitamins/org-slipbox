/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.navigation

import android.os.Build
import androidx.activity.ComponentActivity
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.DeviceConfigurationOverride
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.WindowSize
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.test.espresso.Espresso
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.SdkSuppress
import androidx.test.platform.app.InstrumentationRegistry
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.ui.MemoryStore
import io.github.b_vitamins.slipbox.ui.Specimen
import io.github.b_vitamins.slipbox.ui.regression
import io.github.b_vitamins.slipbox.ui.settings.ReadingSettings
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
@SdkSuppress(minSdkVersion = Build.VERSION_CODES.O)
class AdaptiveNavigationTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    private val settings = ReadingSettings(MemoryStore())
    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val appearance = context.getString(R.string.action_appearance)
    private val appearancePane = context.getString(R.string.appearance_title)
    private var owner: SlipboxBackStack? = null
    private var size by mutableStateOf(COMPACT)

    @Test
    fun resizeChangesOnlyPresentationAndBackStillPopsOneRoute() {
        show()
        composeRule.onNodeWithText(Synthetic.SEARCH).performClick()
        composeRule.onNodeWithText(Synthetic.OPEN).performClick()
        composeRule.onNodeWithText(Specimen.TITLE).assertIsDisplayed()
        composeRule.onNodeWithText(appearance).performClick()
        assertTrue(composeRule.revealing(appearancePane))

        resize(EXPANDED)
        assertTrue("detail state survives the layout transition", composeRule.revealing(appearancePane))
        composeRule.onNodeWithText(Synthetic.LIBRARY).assertIsDisplayed()
        composeRule.onNodeWithText(Synthetic.SEARCHED).assertIsDisplayed()
        composeRule.onNodeWithText(Specimen.TITLE).assertIsDisplayed()
        assertEquals(2, history().entries.size)
        Espresso.pressBack()
        composeRule.waitForIdle()
        assertFalse("Back dismisses the retained sheet first", composeRule.revealing(appearancePane))
        assertEquals(2, history().entries.size)
        composeRule.regression("navigation-list-detail-expanded")

        composeRule.runOnIdle {
            assertTrue(history().open(Synthetic.note(source = Synthetic.BETA)))
        }
        composeRule.waitForIdle()
        assertEquals(3, history().entries.size)
        Espresso.pressBack()
        composeRule.waitForIdle()
        assertEquals("Back removes exactly one detail", 2, history().entries.size)

        resize(COMPACT)
        composeRule.onNodeWithText(Synthetic.LIBRARY).assertDoesNotExist()
        composeRule.onNodeWithText(Specimen.TITLE).assertIsDisplayed()
        assertEquals(2, history().entries.size)
        composeRule.regression("navigation-list-detail-compact")

        Espresso.pressBack()
        composeRule.waitForIdle()
        composeRule.onNodeWithText(Synthetic.SEARCHED).assertIsDisplayed()
        assertEquals(listOf(SlipboxRoute.Library(Synthetic.SEARCHED)), history().entries)
    }

    private fun show() {
        composeRule.setContent {
            DeviceConfigurationOverride(DeviceConfigurationOverride.WindowSize(size)) {
                SyntheticHost(settings, onOwner = { owner = it })
            }
        }
        composeRule.waitForIdle()
    }

    private fun resize(next: DpSize) {
        composeRule.runOnIdle { size = next }
        composeRule.waitForIdle()
    }

    private fun history(): SlipboxBackStack = checkNotNull(owner) { "no destination presented" }

    private companion object {
        val COMPACT = DpSize(320.dp, 640.dp)
        val EXPANDED = DpSize(900.dp, 700.dp)
    }
}
