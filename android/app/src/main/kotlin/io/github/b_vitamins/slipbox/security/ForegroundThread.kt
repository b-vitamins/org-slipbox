/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

/**
 * Whether the calling thread is the one that draws the screen.
 *
 * Keystore and file work must not block it, so a vault asks before it blocks.
 * The question sits behind an interface because the answer is the platform's:
 * `SlipboxVault` supplies the main looper's, and [None] suits a caller, such as
 * a unit test, whose threads are all its own.
 */
fun interface ForegroundThread {

    fun isCurrent(): Boolean

    companion object {

        /** No thread is the foreground one. */
        val None: ForegroundThread = ForegroundThread { false }
    }
}
