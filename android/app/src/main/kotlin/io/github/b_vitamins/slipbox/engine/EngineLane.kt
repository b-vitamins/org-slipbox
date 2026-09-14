/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.engine

import java.util.concurrent.ArrayBlockingQueue
import java.util.concurrent.Callable
import java.util.concurrent.CancellationException
import java.util.concurrent.ExecutionException
import java.util.concurrent.FutureTask
import java.util.concurrent.RejectedExecutionException
import java.util.concurrent.ThreadFactory
import java.util.concurrent.ThreadPoolExecutor
import java.util.concurrent.TimeUnit
import java.util.concurrent.TimeoutException
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger

/** What became of a call [EngineTask.cancel] was asked to abandon. */
enum class EngineCancellation {
    /** The lane had not started it, and now never will. */
    DISCARDED,

    /** Under way: a native query cannot be interrupted, so it runs to its answer. */
    UNDER_WAY,

    /** Already answered, or already abandoned. */
    SETTLED,
}

/** Receives the answer of one call, unless disposal retires it first. */
interface EngineCallback<T> {

    fun onAnswered(answer: T)

    fun onFailed(failure: Throwable)
}

/** One native call on an owner's lane. */
class EngineTask<T> internal constructor(
    private val call: FutureTask<T>,
    /**
     * Separately tracks whether execution began; FutureTask cancellation cannot report that.
     */
    private val claim: AtomicBoolean,
) {

    val isSettled: Boolean
        get() = call.isDone

    /**
     * Blocks until the call settles, and fails with what it failed with. A
     * caller that must not block supplies a callback instead.
     */
    fun await(): T =
        try {
            call.get()
        } catch (failed: ExecutionException) {
            throw failed.cause ?: failed
        }

    /** Blocks for at most [timeoutMillis], answering null if the call is still under way. */
    fun await(timeoutMillis: Long): T? =
        try {
            call.get(timeoutMillis, TimeUnit.MILLISECONDS)
        } catch (unsettled: TimeoutException) {
            null
        } catch (failed: ExecutionException) {
            throw failed.cause ?: failed
        }

    /**
     * Abandons the call if the lane has not started it yet. A started native
     * query is not interrupted, and the answer reports that rather than a
     * cancellation that did not happen.
     */
    fun cancel(): EngineCancellation =
        when {
            call.isDone -> EngineCancellation.SETTLED
            !claim.compareAndSet(false, true) -> EngineCancellation.UNDER_WAY
            call.cancel(false) -> EngineCancellation.DISCARDED
            else -> EngineCancellation.SETTLED
        }

    internal companion object {

        fun <T> settled(answer: T): EngineTask<T> {
            val call = FutureTask(Callable { answer })
            call.run()
            return EngineTask(call, AtomicBoolean(true))
        }

        fun <T> failed(failure: Throwable): EngineTask<T> {
            val call = FutureTask<T>(Callable { throw failure })
            call.run()
            return EngineTask(call, AtomicBoolean(true))
        }
    }
}

/**
 * Serial native execution with bounded admission and reserved closure capacity.
 * Cancellation releases both physical queue space and its admission count.
 */
internal class EngineLane(name: String, private val depth: Int, capacity: Int) {

    private val executor =
        ThreadPoolExecutor(
            1,
            1,
            0L,
            TimeUnit.MILLISECONDS,
            ArrayBlockingQueue(capacity),
            ThreadFactory { runnable -> Thread(runnable, name).apply { isDaemon = true } },
        )

    private val admitted = AtomicInteger()

    private var retired = false

    @Synchronized
    fun <T> submit(work: () -> T): EngineTask<T> {
        ensureLive()
        if (admitted.get() >= depth) {
            throw saturated()
        }
        admitted.incrementAndGet()
        val claim = AtomicBoolean()
        val call = Counted(Callable { claimed(claim, work) })
        try {
            executor.execute(call)
        } catch (rejected: RejectedExecutionException) {
            admitted.decrementAndGet()
            throw saturated()
        }
        return EngineTask(call, claim)
    }

    /** Closures are ordered behind the work they retire and cannot be abandoned. */
    @Synchronized
    fun <T> submitClosure(work: () -> T): EngineTask<T> = ordered(work, retired)

    /**
     * Order disposal after admitted work. A repeated retirement adds no work.
     */
    @Synchronized
    fun <T> retire(work: () -> T): EngineTask<T> {
        val repeat = retired
        retired = true
        return ordered(work, repeat).also { executor.shutdown() }
    }

    /** True once the lane's own thread has finished everything it holds. */
    fun awaitRetirement(timeoutMillis: Long): Boolean =
        executor.awaitTermination(timeoutMillis, TimeUnit.MILLISECONDS)

    /** How many calls the lane has admitted and not yet settled. */
    internal fun admitted(): Int = admitted.get()

    /** Submits work the queue has reserved room for, so it is never abandoned. */
    private fun <T> ordered(work: () -> T, retired: Boolean): EngineTask<T> {
        if (retired) {
            return EngineTask.failed(EngineClosedException(RETIRED_LANE))
        }
        val call = FutureTask(Callable(work))
        return try {
            executor.execute(call)
            EngineTask(call, AtomicBoolean(true))
        } catch (rejected: RejectedExecutionException) {
            EngineTask.failed(saturated())
        }
    }

    private fun ensureLive() {
        if (retired) {
            throw EngineClosedException(RETIRED_LANE)
        }
    }

    /** Runs [work] unless a cancellation claimed the call first. */
    private fun <T> claimed(claim: AtomicBoolean, work: () -> T): T {
        if (!claim.compareAndSet(false, true)) {
            throw CancellationException("the call was abandoned before it crossed")
        }
        return work()
    }

    private fun saturated(): EngineRefusedException =
        EngineRefusedException(
            AdapterResponse.Refused(
                reason = RefusalReason.OUT_OF_BOUNDS,
                bound = AdapterBound.QUEUED_REQUESTS,
                version = ADAPTER_PROTOCOL_VERSION,
            ),
        )

    /**
     * Remove cancelled calls from the queue before releasing their admission slot.
     */
    private inner class Counted<T>(body: Callable<T>) : FutureTask<T>(body) {

        override fun done() {
            if (isCancelled) {
                executor.remove(this)
            }
            admitted.decrementAndGet()
        }
    }

    private companion object {

        const val RETIRED_LANE = "the owner's lane has been retired"
    }
}
