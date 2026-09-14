/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.Collections
import java.util.concurrent.CountDownLatch
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger

/** The vault's own thread: who runs the work, in what order, and who hears it. */
class VaultSessionTest {

    private val vaults = OpenVaults()
    private val keys = FakeVaultKeys()
    private val file = FakeRecordFile()
    private val foreground = FakeForegroundThread()
    private val vault = vaults.open(keys = keys, file = file, foreground = foreground).completed()

    @After
    fun closeTheVaults() {
        vaults.closeAll()
    }

    @Test
    fun anAsynchronousReadAnswersItsRecipientOnTheVaultThread() {
        val credential = StoredCredential(syntheticToken("access"))
        vault.replace(credential).completed()

        val answers = LinkedBlockingQueue<Pair<String, VaultOutcome<StoredCredential?>>>()
        vault.readAsync(
            VaultRecipient { outcome -> answers.add(Thread.currentThread().name to outcome) },
        )

        val (thread, outcome) = requireNotNull(answers.poll(TIMEOUT_SECONDS, TimeUnit.SECONDS))
        assertEquals(credential, outcome.completed())
        assertNotEquals(Thread.currentThread().name, thread)
    }

    @Test
    fun aDisposedRecipientNeverReceivesACredential() {
        vault.replace(StoredCredential(syntheticToken("access"))).completed()

        val delivered = AtomicInteger()
        val recipient = VaultRecipient<StoredCredential?> { delivered.incrementAndGet() }
        recipient.dispose()
        assertFalse(recipient.isLive)

        vault.readAsync(recipient)
        vault.close()

        assertTrue(vault.awaitClosed(TIMEOUT_MILLIS))
        assertEquals(0L, delivered.get().toLong())
    }

    @Test
    fun aRecipientIsAnsweredOnceHoweverManyRequestsItIsGiven() {
        val delivered = AtomicInteger()
        val recipient = VaultRecipient<StoredCredential?> { delivered.incrementAndGet() }

        vault.readAsync(recipient)
        vault.readAsync(recipient)
        vault.close()

        assertTrue(vault.awaitClosed(TIMEOUT_MILLIS))
        assertEquals(1L, delivered.get().toLong())
    }

    @Test
    fun requestsAreAnsweredInTheOrderTheyWereAccepted() {
        val order = Collections.synchronizedList(mutableListOf<Int>())
        for (position in 1..REQUESTS) {
            vault.replaceAsync(
                StoredCredential(syntheticToken("access-$position")),
                VaultRecipient { order.add(position) },
            )
        }
        vault.close()

        assertTrue(vault.awaitClosed(TIMEOUT_MILLIS))
        assertEquals((1..REQUESTS).toList(), order.toList())
        assertEquals(REQUESTS.toLong(), file.attempts.toLong())
    }

    @Test
    fun theForegroundThreadMayAskAsLongAsItDoesNotWait() {
        foreground.current = true

        val answers = LinkedBlockingQueue<VaultOutcome<StoredCredential?>>()
        vault.readAsync(VaultRecipient { answers.add(it) })

        val outcome = requireNotNull(answers.poll(TIMEOUT_SECONDS, TimeUnit.SECONDS))
        assertNull(outcome.completed())
    }

    @Test
    fun aRequestAfterCloseIsRefusedToItsRecipient() {
        vault.close()

        val answers = LinkedBlockingQueue<VaultOutcome<StoredCredential?>>()
        vault.readAsync(VaultRecipient { answers.add(it) })

        val outcome = requireNotNull(answers.poll(TIMEOUT_SECONDS, TimeUnit.SECONDS))
        assertEquals(VaultFailure.VaultClosed, outcome.refusal())
    }

    @Test
    fun workAcceptedBeforeCloseFinishesAndStillAnswers() {
        val session = VaultSession(FakeForegroundThread())
        val started = CountDownLatch(1)
        val release = CountDownLatch(1)
        val answers = LinkedBlockingQueue<VaultOutcome<Int>>()

        session.submit(
            {
                started.countDown()
                release.await()
                VaultOutcome.Completed(1)
            },
            VaultRecipient { answers.add(it) },
        )
        assertTrue(started.await(TIMEOUT_SECONDS, TimeUnit.SECONDS))
        session.close()
        assertTrue(session.isClosed)

        val refused = LinkedBlockingQueue<VaultOutcome<Int>>()
        session.submit({ VaultOutcome.Completed(2) }, VaultRecipient { refused.add(it) })
        assertEquals(
            VaultFailure.VaultClosed,
            requireNotNull(refused.poll(TIMEOUT_SECONDS, TimeUnit.SECONDS)).refusal(),
        )

        release.countDown()
        assertTrue(session.awaitClosed(TIMEOUT_MILLIS))
        val answered = requireNotNull(answers.poll(TIMEOUT_SECONDS, TimeUnit.SECONDS))
        assertEquals(1L, answered.completed().toLong())
    }

    @Test
    fun workThatThrowsIsReportedWithoutItsMessage() {
        val session = VaultSession(FakeForegroundThread())
        try {
            val awaited =
                session.await<Int> { throw IllegalStateException(QUOTED_INPUT) }.refusal()
            assertEquals(VaultFailure.WorkFailed("java.lang.IllegalStateException"), awaited)
            assertFalse(awaited.toString(), awaited.toString().contains(QUOTED_INPUT))

            val answers = LinkedBlockingQueue<VaultOutcome<Int>>()
            session.submit<Int>(
                { throw IllegalStateException(QUOTED_INPUT) },
                VaultRecipient { answers.add(it) },
            )
            val submitted = requireNotNull(answers.poll(TIMEOUT_SECONDS, TimeUnit.SECONDS)).refusal()
            assertEquals(VaultFailure.WorkFailed("java.lang.IllegalStateException"), submitted)
        } finally {
            session.close()
        }
    }

    @Test
    fun aBlockingCallFromTheForegroundThreadNeverReachesTheWork() {
        val session = VaultSession(FakeForegroundThread(current = true))
        try {
            var ran = false
            val outcome =
                session.await {
                    ran = true
                    VaultOutcome.Completed(Unit)
                }
            assertEquals(VaultFailure.ForegroundRefused, outcome.refusal())
            assertFalse("the work ran on the foreground thread", ran)
        } finally {
            session.close()
        }
    }

    @Test
    fun aRequestBeyondThePendingLimitIsRefusedRatherThanQueuedWithoutBound() {
        val session = VaultSession(FakeForegroundThread())
        val release = CountDownLatch(1)
        val answers = LinkedBlockingQueue<VaultOutcome<Int>>()
        try {
            fill(session, release, answers)

            val refused = LinkedBlockingQueue<VaultOutcome<Int>>()
            session.submit({ VaultOutcome.Completed(0) }, VaultRecipient { refused.add(it) })

            assertEquals(
                VaultFailure.VaultAtCapacity(VaultBound.PendingRequests),
                requireNotNull(refused.poll(TIMEOUT_SECONDS, TimeUnit.SECONDS)).refusal(),
            )
        } finally {
            release.countDown()
            session.close()
        }
        assertTrue(session.awaitClosed(TIMEOUT_MILLIS))
        assertEquals(ACCEPTED_AT_CAPACITY.toLong(), answers.size.toLong())
    }

    @Test
    fun aBlockingCallThatWouldJoinAFullQueueIsRefusedRatherThanWaiting() {
        val session = VaultSession(FakeForegroundThread())
        val release = CountDownLatch(1)
        val answers = LinkedBlockingQueue<VaultOutcome<Int>>()
        try {
            fill(session, release, answers)
            var ran = false

            val outcome =
                session.await {
                    ran = true
                    VaultOutcome.Completed(0)
                }

            assertEquals(
                VaultFailure.VaultAtCapacity(VaultBound.PendingRequests),
                outcome.refusal(),
            )
            assertFalse("a refused request ran anyway", ran)
        } finally {
            release.countDown()
            session.close()
        }
        assertTrue(session.awaitClosed(TIMEOUT_MILLIS))
    }

    @Test
    fun anObserverThatThrowsOnACapacityRefusalIsContainedRatherThanThrown() {
        val thrown = IllegalStateException::class.java.name
        val throwing = FakeDeliveryFaults { IllegalStateException(QUOTED_INPUT) }
        val session = VaultSession(FakeForegroundThread(), throwing)
        val release = CountDownLatch(1)
        val answers = LinkedBlockingQueue<VaultOutcome<Int>>()
        try {
            fill(session, release, answers)
            val refused = VaultRecipient<Int> { throw IllegalStateException(QUOTED_INPUT) }

            answered { session.submit({ VaultOutcome.Completed(0) }, refused) }

            assertEquals(listOf(thrown), throwing.origins)
        } finally {
            release.countDown()
            session.close()
        }
        assertTrue(session.awaitClosed(TIMEOUT_MILLIS))
    }

    /**
     * Occupies [session] with one request that waits for [release] and fills its
     * queue behind that one, so the next request is the one over the bound.
     */
    private fun fill(
        session: VaultSession,
        release: CountDownLatch,
        answers: LinkedBlockingQueue<VaultOutcome<Int>>,
    ) {
        val started = CountDownLatch(1)
        session.submit(
            {
                started.countDown()
                release.await()
                VaultOutcome.Completed(0)
            },
            VaultRecipient { answers.add(it) },
        )
        assertTrue(started.await(TIMEOUT_SECONDS, TimeUnit.SECONDS))
        repeat(VaultSession.PENDING_LIMIT) {
            session.submit({ VaultOutcome.Completed(0) }, VaultRecipient { answers.add(it) })
        }
    }

    private companion object {

        const val REQUESTS = 5

        /** The one being run and every one the queue holds behind it. */
        const val ACCEPTED_AT_CAPACITY = 1 + VaultSession.PENDING_LIMIT

        const val TIMEOUT_SECONDS = 5L

        const val TIMEOUT_MILLIS = 5_000L
    }
}
