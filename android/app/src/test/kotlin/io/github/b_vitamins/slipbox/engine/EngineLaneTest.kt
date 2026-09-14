/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.engine

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.concurrent.CancellationException
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

class EngineLaneTest {

    private val gate = CountDownLatch(1)

    private val arrived = CountDownLatch(1)

    private val lanes = mutableListOf<EngineLane>()

    @After
    fun retireLanes() {
        gate.countDown()
        for (lane in lanes) {
            lane.retire { }
            assertTrue(lane.awaitRetirement(TIMEOUT_MILLIS))
        }
    }

    @Test
    fun aCallRunsOnTheLanesOwnThreadAndNotTheCallers() {
        val lane = lane(2, 5)

        val task = lane.submit { Thread.currentThread().name }

        assertEquals(LANE_NAME, task.await(TIMEOUT_MILLIS))
        assertNotEquals(Thread.currentThread().name, task.await())
    }

    @Test
    fun aCallBeyondTheDeclaredDepthIsRefusedRatherThanQueued() {
        val lane = lane(2, 8)
        lane.block()
        val queued = lane.submit { ANSWER }
        assertEquals(2, lane.admitted())

        val refusal = assertThrows(EngineRefusedException::class.java) { lane.submit { ANSWER } }

        assertEquals(RefusalReason.OUT_OF_BOUNDS, refusal.refusal.reason)
        assertEquals(AdapterBound.QUEUED_REQUESTS, refusal.refusal.bound)
        assertEquals(2, lane.admitted())
        gate.countDown()
        assertEquals(ANSWER, queued.await(TIMEOUT_MILLIS))
    }

    @Test
    fun anAdmissionTheQueueWillNotTakeIsRolledBack() {
        val lane = lane(4, 1)
        lane.block()
        lane.submit { ANSWER }

        val refusal = assertThrows(EngineRefusedException::class.java) { lane.submit { ANSWER } }

        assertEquals(AdapterBound.QUEUED_REQUESTS, refusal.refusal.bound)
        assertEquals(2, lane.admitted())
    }

    @Test
    fun anAbandonedCallLeavesTheQueueItOccupied() {
        val lane = lane(2, 3)
        lane.block()

        repeat(CHURN) { round ->
            val abandoned = lane.submit { round }
            assertEquals(round.toString(), EngineCancellation.DISCARDED, abandoned.cancel())
            assertEquals(round.toString(), 1, lane.admitted())
        }

        val queued = lane.submit { ANSWER }
        gate.countDown()
        assertEquals(ANSWER, queued.await(TIMEOUT_MILLIS))
    }

    @Test
    fun aClosureIsOrderedBehindTheWorkItRetiresAndIsNeverRefused() {
        val lane = lane(2, 5)
        val order = CopyOnWriteArrayList<String>()
        lane.block()
        lane.submit { order += "call" }
        assertThrows(EngineRefusedException::class.java) { lane.submit { ANSWER } }

        val closures = (1..3).map { closure -> lane.submitClosure { order += "closure $closure" } }

        gate.countDown()
        for (closure in closures) {
            assertTrue(closure.await(TIMEOUT_MILLIS) != null)
        }
        assertEquals(listOf("call", "closure 1", "closure 2", "closure 3"), order.toList())
    }

    @Test
    fun aStartedCallIsUnderWayAndAnAbandonedOneIsNeverRun() {
        val lane = lane(2, 5)
        val ran = CopyOnWriteArrayList<String>()
        val blocked = lane.block()
        val queued = lane.submit { ran += "queued" }

        assertEquals(EngineCancellation.DISCARDED, queued.cancel())
        assertEquals(EngineCancellation.SETTLED, queued.cancel())
        assertEquals(EngineCancellation.UNDER_WAY, blocked.cancel())

        gate.countDown()
        assertEquals(true, blocked.await(TIMEOUT_MILLIS))
        assertEquals(EngineCancellation.SETTLED, blocked.cancel())
        assertThrows(CancellationException::class.java) { queued.await() }
        assertEquals(emptyList<String>(), ran.toList())
    }

    @Test
    fun aRetiredLaneRefusesFurtherWorkAndARepeatRetiresNothing() {
        val lane = lane(2, 5)

        val retirement = lane.retire { ANSWER }

        assertEquals(ANSWER, retirement.await(TIMEOUT_MILLIS))
        assertTrue(lane.awaitRetirement(TIMEOUT_MILLIS))
        assertThrows(EngineClosedException::class.java) { lane.submit { ANSWER } }
        assertThrows(EngineClosedException::class.java) { lane.retire { ANSWER }.await() }
        assertThrows(EngineClosedException::class.java) { lane.submitClosure { ANSWER }.await() }
    }

    private fun lane(depth: Int, capacity: Int): EngineLane =
        EngineLane(LANE_NAME, depth, capacity).also { lanes += it }

    /** Occupies the lane's one thread until the gate opens. */
    private fun EngineLane.block(): EngineTask<Boolean> {
        val blocked =
            submit {
                arrived.countDown()
                gate.await(TIMEOUT_MILLIS, TimeUnit.MILLISECONDS)
            }
        assertTrue(arrived.await(TIMEOUT_MILLIS, TimeUnit.MILLISECONDS))
        return blocked
    }

    private companion object {

        const val LANE_NAME = "test-lane"

        const val ANSWER = "answered"

        const val TIMEOUT_MILLIS = 5_000L

        /** Far more abandoned calls than the queue could ever hold at once. */
        const val CHURN = 200
    }
}
