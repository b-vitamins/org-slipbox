/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth

/** Monotonic time for polling; wall time for persisted token expiry. */
internal interface AuthorizationClock {


    fun elapsedMillis(): Long


    fun epochSeconds(): Long


    fun waitFor(millis: Long): Boolean
}

internal object SystemAuthorizationClock : AuthorizationClock {

    private const val MILLIS_PER_SECOND = 1_000L

    private const val NANOS_PER_MILLI = 1_000_000L

    override fun elapsedMillis(): Long = System.nanoTime() / NANOS_PER_MILLI

    override fun epochSeconds(): Long = System.currentTimeMillis() / MILLIS_PER_SECOND

    override fun waitFor(millis: Long): Boolean =
        try {
            Thread.sleep(millis)
            true
        } catch (interrupted: InterruptedException) {
            false
        }
}
