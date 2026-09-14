/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.theme

import android.content.ContentResolver
import android.database.ContentObserver
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import kotlin.math.roundToInt

/** The web surface's `--ease-settle`. */
internal val SlipboxSettle =
    CubicBezierEasing(
        SlipboxTokens.Motion.SETTLE_X1,
        SlipboxTokens.Motion.SETTLE_Y1,
        SlipboxTokens.Motion.SETTLE_X2,
        SlipboxTokens.Motion.SETTLE_Y2,
    )

/** Explicit reduction or a zero platform animator scale disables nonessential motion. */
@Immutable
internal data class SlipboxMotion(
    val platformScale: Float = 1f,
    val reduceMotion: Boolean = false,
) {
    val reduced: Boolean
        get() = reduceMotion || platformScale <= 0f

    /** Compose applies the platform scale itself; do not multiply it here. */
    fun native(durationMs: Int): Int = if (reduced) 0 else durationMs

    /** Documents have no platform animator scale; apply it here once. */
    fun document(durationMs: Int): Int =
        if (reduced) 0 else (durationMs * platformScale).roundToInt()
}

/** Observe animator scale per composition and unregister on disposal. */
@Composable
internal fun rememberPlatformMotionScale(): Float {
    val resolver = LocalContext.current.contentResolver
    var scale by remember(resolver) { mutableFloatStateOf(animatorDurationScale(resolver)) }
    DisposableEffect(resolver) {
        val observer =
            object : ContentObserver(Handler(Looper.getMainLooper())) {
                override fun onChange(selfChange: Boolean) {
                    scale = animatorDurationScale(resolver)
                }
            }
        resolver.registerContentObserver(
            Settings.Global.getUriFor(Settings.Global.ANIMATOR_DURATION_SCALE),
            false,
            observer,
        )
        onDispose { resolver.unregisterContentObserver(observer) }
    }
    return scale
}

private fun animatorDurationScale(resolver: ContentResolver): Float =
    Settings.Global
        .getFloat(resolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f)
        .coerceAtLeast(0f)
