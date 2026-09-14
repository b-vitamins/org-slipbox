/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import android.os.Build
import android.provider.Settings
import androidx.activity.ComponentActivity
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.SdkSuppress
import androidx.test.platform.app.InstrumentationRegistry
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.ui.document.DocumentPresentation
import io.github.b_vitamins.slipbox.ui.document.rememberDocumentPresentation
import io.github.b_vitamins.slipbox.ui.settings.ReadingPreferences
import io.github.b_vitamins.slipbox.ui.settings.ReadingPreferencesRecord
import io.github.b_vitamins.slipbox.ui.settings.ReadingSettings
import io.github.b_vitamins.slipbox.ui.theme.SlipboxAppearance
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTheme
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTokens
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/** How the surface moves: its own settle, the reader's refusal and the platform's. */
@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
@SdkSuppress(minSdkVersion = Build.VERSION_CODES.Q)
class MotionTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val context = instrumentation.targetContext
    private val control = context.getString(R.string.action_appearance)
    private val paneTitle = context.getString(R.string.appearance_title)
    private var previousScale: String? = null
    private var toldColumnMs = -1

    /** Changing one synthetic device setting takes the shell's own identity. */
    @Before
    fun adoptTheShellIdentity() {
        instrumentation.uiAutomation.adoptShellPermissionIdentity()
        previousScale =
            Settings.Global.getString(
                context.contentResolver,
                Settings.Global.ANIMATOR_DURATION_SCALE,
            )
    }

    @After
    fun restoreTheSettingAndDropTheIdentity() {
        Settings.Global.putString(
            context.contentResolver,
            Settings.Global.ANIMATOR_DURATION_SCALE,
            previousScale,
        )
        instrumentation.uiAutomation.dropShellPermissionIdentity()
    }

    @Test
    fun theRevealSettlesIntoItsPlaceInsteadOfAppearingInIt() {
        show(reduceMotion = false)
        composeRule.mainClock.autoAdvance = false
        composeRule.onNodeWithText(control).performClick()
        composeRule.mainClock.advanceTimeByFrame()
        val start = sheetTop()
        composeRule.mainClock.advanceTimeBy(STEP)
        val moving = sheetTop()
        composeRule.mainClock.advanceTimeBy(SETTLED)
        val rest = sheetTop()
        composeRule.mainClock.advanceTimeBy(SETTLED)
        assertEquals("the reveal comes to rest", rest, sheetTop(), 0f)
        assertTrue("it starts below its place: $start of $rest", start > rest)
        assertTrue("it moves towards it: $moving of $rest..$start", moving in rest..start)
        assertEquals(
            "a document is told the same duration",
            SlipboxTokens.Motion.COLUMN_MS,
            toldColumnMs,
        )
        Evidence.record(
            "motion-settle",
            Record()
                .size("firstFramePx", start)
                .size("movingPx", moving)
                .size("restingPx", rest)
                .count("columnMs", toldColumnMs),
        )
    }

    @Test
    fun aReaderWhoAsksForLessMotionFindsTheRevealAlreadyInPlace() {
        show(reduceMotion = true)
        composeRule.mainClock.autoAdvance = false
        composeRule.onNodeWithText(control).performClick()
        composeRule.mainClock.advanceTimeByFrame()
        val start = sheetTop()
        composeRule.mainClock.advanceTimeBy(SETTLED)
        assertEquals("nothing moved after it", start, sheetTop(), 1f)
        assertEquals("a document is told to move nothing", 0, toldColumnMs)
    }

    @Test
    fun thePlatformsScaleReachesWhatAMountedDocumentIsTold() {
        val seen = mutableListOf<DocumentPresentation>()
        composeRule.setContent { SlipboxTheme { MotionProbe(seen) } }
        scale(1f)
        composeRule.waitUntil(TIMEOUT) {
            seen.last().motion.columnMs == SlipboxTokens.Motion.COLUMN_MS
        }
        val ordinary = exported("ordinary", seen.last())
        assertFalse("an ordinary scale is no refusal", ordinary.motion.reduced)

        scale(2f)
        val scaled = 2 * SlipboxTokens.Motion.COLUMN_MS
        composeRule.waitUntil(TIMEOUT) { seen.last().motion.columnMs == scaled }
        val doubled = exported("scaled", seen.last())
        assertFalse("nor is a doubled one", doubled.motion.reduced)

        scale(0f)
        composeRule.waitUntil(TIMEOUT) { seen.last().motion.reduced }
        val refused = exported("refused", seen.last())
        assertEquals("a refusal is the whole of it", 0, refused.motion.columnMs)
        assertEquals(
            "and reaches the durations alone, not the type",
            ordinary.typography,
            refused.typography,
        )
        Evidence.record(
            "motion-platform",
            Record()
                .count("tokenMs", SlipboxTokens.Motion.COLUMN_MS)
                .count("scaledMs", scaled)
                .count("removedMs", refused.motion.columnMs)
                .count("changesSeen", seen.size),
        )
    }

    @Test
    fun aRetiredSubtreeIsToldNothingFurther() {
        val kept = mutableListOf<DocumentPresentation>()
        val retired = mutableListOf<DocumentPresentation>()
        var mounted by mutableStateOf(true)
        composeRule.setContent {
            SlipboxTheme {
                if (mounted) MotionProbe(retired)
                MotionProbe(kept)
            }
        }
        scale(1f)
        composeRule.waitUntil(TIMEOUT) {
            kept.last().motion.columnMs == SlipboxTokens.Motion.COLUMN_MS
        }

        scale(2f)
        val scaled = 2 * SlipboxTokens.Motion.COLUMN_MS
        composeRule.waitUntil(TIMEOUT) { kept.last().motion.columnMs == scaled }
        composeRule.waitUntil(TIMEOUT) { retired.last().motion.columnMs == scaled }

        composeRule.runOnIdle { mounted = false }
        composeRule.waitForIdle()
        val last = retired.last()

        scale(3f)
        val further = 3 * SlipboxTokens.Motion.COLUMN_MS
        composeRule.waitUntil(TIMEOUT) { kept.last().motion.columnMs == further }
        composeRule.waitForIdle()
        assertEquals("the retired subtree was left as it was", last, retired.last())
        assertFalse("it was told nothing further", retired.any { it.motion.columnMs == further })
    }

    @Composable
    private fun MotionProbe(seen: MutableList<DocumentPresentation>) {
        val presentation =
            rememberDocumentPresentation(
                appearance = SlipboxAppearance.System,
                reduceMotion = false,
                availableWidth = LIGHT.width,
            )
        SideEffect { seen += presentation }
    }

    /**
     * One motion state, exported as a document mounted in it would be handed it. The
     * harness reads the durations it drives its own reveal with out of these files.
     */
    private fun exported(state: String, presentation: DocumentPresentation): DocumentPresentation {
        Evidence.record(
            "presentation-motion-$state",
            Record().raw("presentation", presentation.toJson()),
        )
        return presentation
    }

    private fun show(reduceMotion: Boolean) {
        val preferences = ReadingPreferences(reduceMotion = reduceMotion)
        val settings =
            ReadingSettings(MemoryStore(record = ReadingPreferencesRecord.Stored(preferences)))
        composeRule.setContent {
            LIGHT.Content {
                val presentation =
                    rememberDocumentPresentation(
                        appearance = SlipboxAppearance.System,
                        reduceMotion = reduceMotion,
                        availableWidth = LIGHT.width,
                    )
                toldColumnMs = presentation.motion.columnMs
                SpecimenNote(settings = settings)
            }
        }
        composeRule.waitForIdle()
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

    private companion object {
        const val TIMEOUT = 5_000L
        val STEP = SlipboxTokens.Motion.COLUMN_MS / 4L
        val SETTLED = 2L * SlipboxTokens.Motion.COLUMN_MS
        val LIGHT = VISUAL_CASES.first { !it.dark && it.fontScale == 1f }
    }
}
