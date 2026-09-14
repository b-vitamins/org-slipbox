/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import android.os.Build
import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.PixelMap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.SdkSuppress
import androidx.test.platform.app.InstrumentationRegistry
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.ui.settings.ReadingSettings
import io.github.b_vitamins.slipbox.ui.theme.SlipboxAppearance
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/** Two reading surfaces at once: what one of them does must not reach the other. */
@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
@SdkSuppress(minSdkVersion = Build.VERSION_CODES.O)
class MountsTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val control = context.getString(R.string.action_appearance)
    private val followSystem = context.getString(R.string.appearance_system)

    private val created = mutableListOf<String>()
    private val retired = mutableListOf<String>()

    private var firstScheme by mutableStateOf(SlipboxAppearance.Light)
    private var secondScheme by mutableStateOf(SlipboxAppearance.Dark)
    private var firstMounted by mutableStateOf(true)

    @Test
    fun twoMountsPaintTheirOwnSchemesAtOnce() {
        show()
        val first = surface(FIRST)
        val second = surface(SECOND)
        assertEquals("the first", hex(paper(dark = false)), hex(first[2, 2]))
        assertEquals("the second", hex(paper(dark = true)), hex(second[2, 2]))
        Evidence.image("mounts-two-schemes", composeRule.onRoot().captureToImage())
        Evidence.record(
            "mounts-two-schemes",
            Record()
                .colour("firstPaper", first[2, 2])
                .colour("secondPaper", second[2, 2])
                .count("firstWidthPx", first.width)
                .count("secondWidthPx", second.width),
        )
    }

    @Test
    fun updatingOneMountLeavesTheOtherAsItWas() {
        show()
        val before = surface(SECOND)
        composeRule.runOnIdle { firstScheme = SlipboxAppearance.Dark }
        composeRule.waitForIdle()
        assertEquals("the update landed", hex(paper(dark = true)), hex(surface(FIRST)[2, 2]))
        assertTrue("the other mount was not repainted", surface(SECOND).matches(before))
        assertEquals("nor built again", 1, created.count { it == SECOND })
        assertEquals("nor retired", 0, retired.count { it == SECOND })
    }

    @Test
    fun aScrollOffsetSurvivesAPresentationOnlyUpdate() {
        show()
        val math = node(Specimen.DISPLAY, SECOND)
        math.performScrollTo()
        val before = math.bounds().top
        composeRule.runOnIdle { secondScheme = SlipboxAppearance.Light }
        composeRule.waitForIdle()
        assertEquals("the scheme changed", hex(paper(dark = false)), hex(surface(SECOND)[2, 2]))
        assertEquals(
            "the place in the note was kept",
            before,
            node(Specimen.DISPLAY, SECOND).bounds().top,
            0f,
        )
        assertEquals("the mount was not built again", 1, created.count { it == SECOND })
    }

    @Test
    fun retiringOneMountLeavesTheOtherStanding() {
        show()
        composeRule.onNode(hasText(control) and hasAnyAncestor(hasTestTag(SECOND))).performClick()
        composeRule.waitForIdle()
        composeRule.runOnIdle { firstMounted = false }
        composeRule.waitForIdle()
        assertEquals("the retired mount was retired once", 1, retired.count { it == FIRST })
        assertEquals("the standing mount was not", 0, retired.count { it == SECOND })
        composeRule.onNodeWithTag(FIRST).assertDoesNotExist()
        composeRule
            .onNode(hasText(followSystem) and hasAnyAncestor(hasTestTag(SECOND)))
            .assertIsDisplayed()
    }

    private fun show() {
        composeRule.setContent {
            WIDE.Overridden {
                Row(modifier = Modifier.fillMaxSize()) {
                    if (firstMounted) {
                        Mount(FIRST, firstScheme, Modifier.weight(1f))
                    }
                    Mount(SECOND, secondScheme, Modifier.weight(1f))
                }
            }
        }
        composeRule.waitForIdle()
    }

    @Composable
    private fun Mount(tag: String, appearance: SlipboxAppearance, modifier: Modifier = Modifier) {
        Box(modifier = modifier.fillMaxHeight().testTag(tag)) {
            SlipboxTheme(appearance = appearance) {
                val settings = remember { ReadingSettings(MemoryStore()) }
                DisposableEffect(Unit) {
                    created += tag
                    onDispose { retired += tag }
                }
                SpecimenNote(settings = settings)
            }
        }
    }

    private fun node(tag: String, mount: String): SemanticsNodeInteraction =
        composeRule.onNode(hasTestTag(tag) and hasAnyAncestor(hasTestTag(mount)))

    private fun surface(mount: String): PixelMap =
        composeRule.onNodeWithTag(mount).captureToImage().pixels()

    private companion object {
        const val FIRST = "mount-one"
        const val SECOND = "mount-two"
        val WIDE = VISUAL_CASES.last { !it.dark && it.fontScale == 1f }
    }
}
