/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui

import android.os.Build
import androidx.activity.ComponentActivity
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedContentTransitionScope
import androidx.compose.animation.ContentTransform
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertHasNoClickAction
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.Dp
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.SdkSuppress
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.ui.document.DocumentPresentation
import io.github.b_vitamins.slipbox.ui.document.documentPresentation
import io.github.b_vitamins.slipbox.ui.document.rememberDocumentPresentation
import io.github.b_vitamins.slipbox.ui.theme.SlipboxAppearance
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions
import io.github.b_vitamins.slipbox.ui.theme.SlipboxMotion
import io.github.b_vitamins.slipbox.ui.theme.SlipboxSettle
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTheme
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTokens
import io.github.b_vitamins.slipbox.ui.theme.SlipboxTransitions
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Each defect the rest of the suite is meant to catch, built here and put to the same
 * assertion the suite passes, which must refuse it.
 */
@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
@SdkSuppress(minSdkVersion = Build.VERSION_CODES.O)
class NegativeControlTest {

    @get:Rule val composeRule = createAndroidComposeRule<ComponentActivity>()

    private lateinit var density: Density
    private lateinit var exported: DocumentPresentation
    private var composed = false
    private var shown by mutableStateOf(MEASURES.first())

    @Test
    fun anExportThatNeverSawTheScaleIsRefused() {
        show(MEASURES.first())
        val plain = exported.typography
        show(MEASURES.last())
        assertTrue("the scale reaches the export", exported.typography.bodySize > plain.bodySize)

        val unscaled = exportedFrom(Density(density.density)).typography
        refuted("an export the scale never reached") {
            assertTrue(unscaled.bodySize > plain.bodySize)
        }
    }

    @Test
    fun aScaleAppliedAsAMultiplicationIsRefused() {
        show(MEASURES.first())
        val plain = exported.typography
        show(MEASURES.last())
        val real = exported.typography
        assertTrue("the reading size grows by less", real.bodySize < 2 * plain.bodySize)
        assertTrue(
            "and the smallest text gains the most",
            real.codeSize / plain.codeSize > real.h1Size / plain.h1Size,
        )

        val doubled =
            plain.copy(
                bodySize = 2 * plain.bodySize,
                h1Size = 2 * plain.h1Size,
                codeSize = 2 * plain.codeSize,
            )
        refuted("type doubled with the scale") {
            assertTrue(doubled.bodySize < 2 * plain.bodySize)
        }
        refuted("a curve that gains the same everywhere") {
            assertTrue(doubled.codeSize / plain.codeSize > doubled.h1Size / plain.h1Size)
        }
    }

    @Test
    fun aMarkOrATargetOutOfProportionIsRefused() {
        composeRule.setContent {
            MEASURES.first().Content {
                density = LocalDensity.current
                val target = SlipboxDimensions.touchTarget
                val glyph = SlipboxDimensions.glyph
                Column(modifier = Modifier.background(SlipboxTheme.colors.paper)) {
                    Mark(tag = FILLED, target = target, mark = target)
                    Mark(tag = SHRUNKEN, target = glyph, mark = glyph)
                }
            }
        }
        composeRule.waitForIdle()

        val frame = SlipboxDimensions.glyph.pixels(density)
        val filled = composeRule.onNodeWithTag(FILLED).captureToImage().pixels()
        val painted = filled.markBounds(paper(dark = false))
        refuted("a mark drawn to the size of the target around it") {
            assertTrue(painted.width <= frame + 1)
        }

        val target = composeRule.onNodeWithTag(SHRUNKEN).bounds()
        refuted("a target drawn to the size of the mark inside it") {
            assertTrue(target.height >= SlipboxDimensions.touchTarget.pixels(density) - 1f)
        }
    }

    @Test
    fun aCanvasPaintedInTheOtherSchemeIsRefused() {
        composeRule.setContent {
            DARK.Overridden {
                Box(
                    modifier =
                        Modifier.fillMaxSize().background(paper(dark = false)).testTag(CANVAS),
                )
            }
        }
        composeRule.waitForIdle()

        val painted = composeRule.onNodeWithTag(CANVAS).captureToImage().pixels()[2, 2]
        refuted("a canvas painted in the other scheme") {
            assertEquals(hex(paper(dark = true)), hex(painted))
        }
    }

    @Test
    fun aRevealThatMovesWhenAskedNotToIsRefused() {
        var visible by mutableStateOf(false)
        composeRule.setContent {
            MEASURES.first().Content {
                ContextualSheet(
                    title = PANE,
                    visible = visible,
                    // The reader's request, dropped: this is the defect under test.
                    motion = SlipboxMotion(),
                    onDismiss = {},
                ) {
                    Text(text = PANE)
                }
            }
        }
        composeRule.mainClock.autoAdvance = false
        composeRule.runOnUiThread { visible = true }
        composeRule.mainClock.advanceTimeByFrame()
        val start = sheetTop()
        composeRule.mainClock.advanceTimeBy(2L * SlipboxTokens.Motion.COLUMN_MS)
        refuted("a reveal that moved after it was asked not to") {
            assertEquals(start, sheetTop(), 1f)
        }
    }

    @Test
    fun aSurfaceLeftActingUnderARevealIsRefused() {
        composeRule.setContent {
            MEASURES.first().Content {
                ReadingSurface(
                    title = SURFACE,
                    // The reveal stands, and the surface under it was never told: the defect.
                    obscured = false,
                    trailing = { TextControl(label = CONTROL, onClick = {}) },
                    overlay = {
                        ContextualSheet(
                            title = PANE,
                            visible = true,
                            motion = SlipboxMotion(reduceMotion = true),
                            onDismiss = {},
                        ) {
                            Text(text = PANE)
                        }
                    },
                    body = {},
                )
            }
        }
        composeRule.waitForIdle()
        composeRule
            .onNode(SemanticsMatcher.expectValue(SemanticsProperties.PaneTitle, PANE))
            .assertExists()

        refuted("a control that still answers under a reveal") {
            composeRule.onNodeWithText(CONTROL).assertIsNotEnabled()
        }
        refuted("a control that still carries an action under a reveal") {
            composeRule.onNodeWithText(CONTROL).assertHasNoClickAction()
        }
        refuted("a surface still offered to accessibility under a reveal") {
            composeRule.onAllNodes(WITHHELD, useUnmergedTree = true).assertCountEquals(1)
        }
    }

    @Test
    fun anExchangeThatOutlastsItsOwnDurationIsRefused() {
        val forward =
            exchanges(SlipboxMotion()) { crossfadeOver(2 * SlipboxTokens.Motion.CROSSFADE_MS) }
        composeRule.mainClock.autoAdvance = false
        composeRule.runOnUiThread { forward(true) }
        composeRule.mainClock.advanceTimeByFrame()
        composeRule.mainClock.advanceTimeBy(SETTLED)

        composeRule.onNodeWithText(STATED + LEAVING).assertDoesNotExist()
        refuted("an exchange that outlasted its own duration") {
            composeRule.onNodeWithText(DEFECT + LEAVING).assertDoesNotExist()
        }
    }

    @Test
    fun aRefusedExchangeThatIsMerelyShortenedIsRefused() {
        val forward =
            exchanges(SlipboxMotion(reduceMotion = true)) {
                crossfadeOver(SlipboxTokens.Motion.CROSSFADE_MS / 2)
            }
        composeRule.mainClock.autoAdvance = false
        composeRule.runOnUiThread { forward(true) }
        composeRule.mainClock.advanceTimeByFrame()
        composeRule.mainClock.advanceTimeByFrame()

        composeRule.onNodeWithText(STATED + LEAVING).assertDoesNotExist()
        refuted("an exchange shortened rather than removed") {
            composeRule.onNodeWithText(DEFECT + LEAVING).assertDoesNotExist()
        }
    }

    /**
     * The same pair of surfaces exchanged twice over: once under the policy [motion] states
     * and once under [defect]. Both run off the one returned switch, so a single clock
     * measures the stated exchange and the counterfactual against each other.
     */
    private fun exchanges(
        motion: SlipboxMotion,
        defect: () -> ContentTransform,
    ): (Boolean) -> Unit {
        var forward by mutableStateOf(false)
        composeRule.setContent {
            MEASURES.first().Content {
                Column(modifier = Modifier.background(SlipboxTheme.colors.paper)) {
                    Exchange(
                        forward = forward,
                        prefix = STATED,
                        transitionSpec = { SlipboxTransitions.exchange<Boolean>(motion)(this) },
                    )
                    Exchange(forward = forward, prefix = DEFECT, transitionSpec = { defect() })
                }
            }
        }
        composeRule.waitForIdle()
        return { target -> forward = target }
    }

    @Composable
    private fun Exchange(
        forward: Boolean,
        prefix: String,
        transitionSpec: AnimatedContentTransitionScope<Boolean>.() -> ContentTransform,
    ) {
        AnimatedContent(targetState = forward, transitionSpec = transitionSpec) { arriving ->
            Text(text = prefix + if (arriving) ARRIVING else LEAVING)
        }
    }

    @Composable
    private fun Mark(tag: String, target: Dp, mark: Dp) {
        Box(
            modifier = Modifier.size(target).testTag(tag),
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                painter = painterResource(R.drawable.ic_back),
                contentDescription = null,
                tint = SlipboxTheme.colors.ink,
                modifier = Modifier.size(mark),
            )
        }
    }

    private fun show(case: VisualCase) {
        if (composed) {
            composeRule.runOnIdle { shown = case }
        } else {
            shown = case
            composeRule.setContent {
                shown.Content {
                    density = LocalDensity.current
                    exported =
                        rememberDocumentPresentation(
                            appearance = SlipboxAppearance.System,
                            reduceMotion = false,
                            availableWidth = shown.width,
                        )
                }
            }
            composed = true
        }
        composeRule.waitForIdle()
    }

    /** What an export taken from another density would have said instead. */
    private fun exportedFrom(density: Density): DocumentPresentation =
        documentPresentation(
            density = density,
            dark = false,
            motion = SlipboxMotion(),
            availableWidth = shown.width,
        )

    private fun sheetTop(): Float =
        composeRule
            .onNode(SemanticsMatcher.expectValue(SemanticsProperties.PaneTitle, PANE))
            .bounds()
            .top

    /** Runs one claim that must fail, and fails if it does not. */
    private fun refuted(what: String, claim: () -> Unit) {
        try {
            claim()
        } catch (refused: AssertionError) {
            return
        }
        fail("$what was not refused")
    }

    private companion object {
        const val FILLED = "control-filled"
        const val SHRUNKEN = "control-shrunken"
        const val CANVAS = "counterfactual-canvas"
        const val PANE = "Counterfactual reveal"
        const val SURFACE = "Counterfactual surface"
        const val CONTROL = "Counterfactual control"
        const val STATED = "stated-"
        const val DEFECT = "defect-"
        const val ARRIVING = "arriving"
        const val LEAVING = "leaving"

        /** Past the exchange's own duration and well short of twice it. */
        const val SETTLED = SlipboxTokens.Motion.CROSSFADE_MS + 64L

        val MEASURES = VISUAL_CASES.filter { it.width.value == 320f && !it.dark }
        val DARK = VISUAL_CASES.first { it.dark && it.fontScale == 1f }

        /** An exchange built to a duration of its own rather than the surface's. */
        fun crossfadeOver(durationMs: Int): ContentTransform {
            val settle = tween<Float>(durationMs, easing = SlipboxSettle)
            return ContentTransform(fadeIn(settle), fadeOut(settle), sizeTransform = null)
        }
    }
}
