/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.theme

import androidx.compose.animation.AnimatedContentTransitionScope
import androidx.compose.animation.ContentTransform
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.togetherWith
import androidx.compose.ui.unit.IntOffset

/** Spatial navigation that keeps the reading trail perceptible without persistent chrome. */
internal object SlipboxTransitions {

    fun <S> advance(
        motion: SlipboxMotion,
    ): AnimatedContentTransitionScope<S>.() -> ContentTransform = {
        horizontal(motion, AnimatedContentTransitionScope.SlideDirection.Left)
    }

    fun <S> retreat(
        motion: SlipboxMotion,
    ): AnimatedContentTransitionScope<S>.() -> ContentTransform = {
        horizontal(motion, AnimatedContentTransitionScope.SlideDirection.Right)
    }

    /** Predictive Back drives the retreat's progress directly from the system edge gesture. */
    fun <S> draggedRetreat(
        motion: SlipboxMotion,
    ): AnimatedContentTransitionScope<S>.(Int) -> ContentTransform = {
        horizontal(motion, AnimatedContentTransitionScope.SlideDirection.Right)
    }

    private fun <S> AnimatedContentTransitionScope<S>.horizontal(
        motion: SlipboxMotion,
        direction: AnimatedContentTransitionScope.SlideDirection,
    ): ContentTransform {
        val duration = motion.native(SlipboxTokens.Motion.COLUMN_MS)
        if (duration == 0) {
            return ContentTransform(
                targetContentEnter = EnterTransition.None,
                initialContentExit = ExitTransition.None,
                sizeTransform = null,
            )
        }
        val settle = tween<IntOffset>(duration, easing = SlipboxSettle)
        return slideIntoContainer(direction, settle) togetherWith
            slideOutOfContainer(direction, settle)
    }
}
