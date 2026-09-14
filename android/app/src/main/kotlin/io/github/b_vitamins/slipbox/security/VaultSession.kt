/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import java.util.concurrent.ArrayBlockingQueue
import java.util.concurrent.Callable
import java.util.concurrent.ExecutionException
import java.util.concurrent.RejectedExecutionException
import java.util.concurrent.ThreadPoolExecutor
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean

/** Bounded serial admission; close rejects new work and drains accepted work before releasing ownership. */
internal class VaultSession(
    private val foreground: ForegroundThread,
    private val faults: VaultDeliveryFaults = VaultDeliveryFaults.Ignored,
    private val onFinished: () -> Unit = {},
) {

    // ThreadLocal.withInitial is above this app's minimum API.
    private val inside: ThreadLocal<Boolean> =
        object : ThreadLocal<Boolean>() {
            override fun initialValue(): Boolean = false
        }

    private val closed = AtomicBoolean(false)

    private val reentrant: Boolean
        get() = inside.get() == true

    private val worker =
        object : ThreadPoolExecutor(
            1,
            1,
            0L,
            TimeUnit.MILLISECONDS,
            ArrayBlockingQueue(PENDING_LIMIT),
            { runnable -> Thread(runnable, THREAD_NAME).apply { isDaemon = true } },
        ) {
            override fun terminated() {
                onFinished()
            }
        }

    val isClosed: Boolean
        get() = closed.get()

    fun <T> await(work: () -> VaultOutcome<T>): VaultOutcome<T> {
        if (closed.get()) {
            return VaultOutcome.Failed(VaultFailure.VaultClosed)
        }
        if (reentrant) {
            return caught(work)
        }
        if (foreground.isCurrent()) {
            return VaultOutcome.Failed(VaultFailure.ForegroundRefused)
        }
        val pending =
            try {
                worker.submit(Callable { marked(work) })
            } catch (rejected: RejectedExecutionException) {
                return VaultOutcome.Failed(unaccepted())
            }
        return try {
            pending.get()
        } catch (interrupted: InterruptedException) {
            // The work continues on the vault thread; only this wait ended.
            Thread.currentThread().interrupt()
            VaultOutcome.Failed(VaultFailure.WorkFailed(originOf(interrupted)))
        } catch (failed: ExecutionException) {
            VaultOutcome.Failed(VaultFailure.WorkFailed(originOf(failed.cause ?: failed)))
        }
    }

    /** Accepted delivery runs on the worker; immediate refusal runs on the calling thread. */
    fun <T> submit(work: () -> VaultOutcome<T>, recipient: VaultRecipient<T>) {
        if (closed.get()) {
            answer(recipient, VaultOutcome.Failed(VaultFailure.VaultClosed))
            return
        }
        try {
            // Callback reentrancy must not submit and wait on this same worker.
            worker.execute { marked { answer(recipient, caught(work)) } }
        } catch (rejected: RejectedExecutionException) {
            answer(recipient, VaultOutcome.Failed(unaccepted()))
        }
    }

    fun close() {
        if (closed.compareAndSet(false, true)) {
            worker.shutdown()
        }
    }

    /** Waits up to [timeoutMillis] for accepted work to finish after [close]. */
    fun awaitClosed(timeoutMillis: Long): Boolean {
        if (reentrant) {
            // Joining this worker from its callback would deadlock.
            return false
        }
        return try {
            worker.awaitTermination(timeoutMillis, TimeUnit.MILLISECONDS)
        } catch (interrupted: InterruptedException) {
            Thread.currentThread().interrupt()
            false
        }
    }

    private fun <T> marked(work: () -> T): T {
        inside.set(true)
        return try {
            work()
        } finally {
            inside.set(false)
        }
    }

    private fun <T> caught(work: () -> VaultOutcome<T>): VaultOutcome<T> =
        try {
            work()
        } catch (error: Exception) {
            VaultOutcome.Failed(VaultFailure.WorkFailed(originOf(error)))
        }

    private fun <T> answer(recipient: VaultRecipient<T>, outcome: VaultOutcome<T>) {
        try {
            recipient.offer(outcome)
        } catch (fault: Throwable) {
            faults.report(originOf(fault))
        }
    }

    private fun unaccepted(): VaultFailure =
        if (worker.isShutdown) {
            VaultFailure.VaultClosed
        } else {
            VaultFailure.VaultAtCapacity(VaultBound.PendingRequests)
        }

    companion object {

        /** How many requests may wait for the vault thread at once. */
        const val PENDING_LIMIT: Int = 8

        private const val THREAD_NAME = "slipbox-vault"
    }
}
