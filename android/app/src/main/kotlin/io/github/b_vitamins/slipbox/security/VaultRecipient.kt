/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import java.util.concurrent.atomic.AtomicReference
import java.util.concurrent.locks.ReentrantLock

/**
 * Once-only delivery. External [dispose] waits for active delivery; self-disposal does not self-wait.
 * Accepted answers arrive on the worker; immediate refusals arrive on the caller's thread.
 */
class VaultRecipient<T>(deliver: (VaultOutcome<T>) -> Unit) {

    private val target = AtomicReference<((VaultOutcome<T>) -> Unit)?>(deliver)

    private val delivering = ReentrantLock()

    val isLive: Boolean
        get() = target.get() != null

    /** Drops any answer, and waits for one another thread is already delivering. */
    fun dispose() {
        target.set(null)
        // Reentrancy permits callback self-disposal.
        delivering.lock()
        delivering.unlock()
    }

    /** Callback faults propagate to the vault's contained reporting policy. */
    internal fun offer(outcome: VaultOutcome<T>): Boolean {
        delivering.lock()
        try {
            val deliver = target.getAndSet(null) ?: return false
            deliver(outcome)
            return true
        } finally {
            delivering.unlock()
        }
    }
}
