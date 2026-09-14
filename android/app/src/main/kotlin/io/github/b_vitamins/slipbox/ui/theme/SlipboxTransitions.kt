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
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut

/** Restrained surface crossfade, omitted when motion is reduced. */
internal object SlipboxTransitions {

    fun <S> exchange(
        motion: SlipboxMotion,
    ): AnimatedContentTransitionScope<S>.() -> ContentTransform = { crossfade(motion) }

    /** Predictive Back uses the same transform, with progress driven by the gesture. */
    fun <S> draggedExchange(
        motion: SlipboxMotion,
    ): AnimatedContentTransitionScope<S>.(Int) -> ContentTransform = { crossfade(motion) }

    private fun crossfade(motion: SlipboxMotion): ContentTransform {
        val duration = motion.native(SlipboxTokens.Motion.CROSSFADE_MS)
        if (duration == 0) {
            return ContentTransform(
                targetContentEnter = EnterTransition.None,
                initialContentExit = ExitTransition.None,
                sizeTransform = null,
            )
        }
        val settle = tween<Float>(duration, easing = SlipboxSettle)
        // Full-window surfaces need no size animation.
        return ContentTransform(
            targetContentEnter = fadeIn(settle),
            initialContentExit = fadeOut(settle),
            sizeTransform = null,
        )
    }
}
