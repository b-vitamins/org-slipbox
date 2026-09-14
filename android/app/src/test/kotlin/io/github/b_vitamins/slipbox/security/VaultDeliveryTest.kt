/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import org.junit.After
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicReference

/**
 * What an asynchronous answer may and may not do to its owner and its thread.
 *
 * Delivery runs on the vault thread, so a fault in a callback and a blocking call
 * from inside one are the vault's problem to handle, not the caller's to discover.
 */
class VaultDeliveryTest {

    private val vaults = OpenVaults()
    private val keys = FakeVaultKeys()
    private val file = FakeRecordFile()
    private val faults = FakeDeliveryFaults()
    private val vault = vaults.open(keys = keys, file = file, faults = faults).completed()
    private val uncaught = AtomicReference<Throwable?>()
    private var previous: Thread.UncaughtExceptionHandler? = null

    @After
    fun closeTheVaults() {
        previous?.let { Thread.setDefaultUncaughtExceptionHandler(it) }
        vaults.closeAll()
    }

    @Test
    fun disposalDoesNotReturnWhileAnAnswerIsBeingDelivered() {
        val delivering = CountDownLatch(1)
        val delivered = AtomicInteger()
        val recipient =
            VaultRecipient<StoredCredential?> {
                delivering.countDown()
                Thread.sleep(HOLD_MILLIS)
                delivered.incrementAndGet()
            }

        vault.readAsync(recipient)
        assertTrue(delivering.await(WAIT_SECONDS, TimeUnit.SECONDS))
        recipient.dispose()
        val seenAtDisposal = delivered.get()

        assertEquals("an answer arrived after disposal returned", 1L, seenAtDisposal.toLong())
        assertFalse(recipient.isLive)
    }

    @Test
    fun anAnswerIsDeliveredOnceEvenWhenDisposalRacesIt() {
        val delivered = AtomicInteger()
        val recipient = VaultRecipient<StoredCredential?> { delivered.incrementAndGet() }

        vault.readAsync(recipient)
        vault.readAsync(recipient)
        vault.close()
        assertTrue(vault.awaitClosed(TimeUnit.SECONDS.toMillis(WAIT_SECONDS)))
        recipient.dispose()

        assertEquals(1L, delivered.get().toLong())
    }

    @Test
    fun aRecipientMayDisposeOfItselfFromInsideItsOwnDelivery() {
        val returned = CountDownLatch(1)
        val recipient = AtomicReference<VaultRecipient<StoredCredential?>?>()
        recipient.set(
            VaultRecipient {
                recipient.get()?.dispose()
                returned.countDown()
            },
        )

        vault.readAsync(requireNotNull(recipient.get()))

        assertTrue("self-disposal did not return", returned.await(WAIT_SECONDS, TimeUnit.SECONDS))
    }

    @Test
    fun aCallbackThatThrowsIsReportedByTypeAndLeftToNoThread() {
        captureUncaughtFaults()
        val marked = syntheticToken("callback")
        val answered = CountDownLatch(1)
        val recipient =
            VaultRecipient<StoredCredential?> {
                answered.countDown()
                throw IllegalStateException(marked)
            }

        vault.readAsync(recipient)
        assertTrue(answered.await(WAIT_SECONDS, TimeUnit.SECONDS))
        val following = AtomicReference<VaultOutcome<StoredCredential?>?>()
        val next = CountDownLatch(1)
        vault.readAsync(
            VaultRecipient {
                following.set(it)
                next.countDown()
            },
        )

        assertTrue("the vault stopped answering", next.await(WAIT_SECONDS, TimeUnit.SECONDS))
        assertNull("a callback fault reached the thread", uncaught.get()?.javaClass?.name)
        assertNull("the empty vault read a credential", requireNotNull(following.get()).completed())
        assertEquals(listOf(IllegalStateException::class.java.name), faults.origins)
        for (reported in faults.origins) {
            assertFalse(reported, reported.contains(MARKER))
            assertFalse(reported, reported.contains(marked))
        }
    }

    @Test
    fun aCallbackThatThrowsOnARefusedRouteIsReportedTheSameWay() {
        val marked = syntheticToken("refused")
        val recipient = VaultRecipient<StoredCredential?> { throw IllegalStateException(marked) }
        vault.close()

        vault.readAsync(recipient)

        assertEquals(listOf(IllegalStateException::class.java.name), faults.origins)
        assertFalse(faults.origins.toString(), faults.origins.any { it.contains(MARKER) })
    }

    @Test
    fun aFaultAnErrorRatherThanAnExceptionIsAlsoReportedRatherThanThrown() {
        captureUncaughtFaults()
        val answered = CountDownLatch(1)
        val recipient =
            VaultRecipient<StoredCredential?> {
                answered.countDown()
                throw AssertionError(syntheticToken("error"))
            }

        vault.readAsync(recipient)

        assertTrue(answered.await(WAIT_SECONDS, TimeUnit.SECONDS))
        assertTrue(vault.awaitClosedAfterClose())
        assertEquals(listOf(AssertionError::class.java.name), faults.origins)
        assertNull("an error reached the thread", uncaught.get()?.javaClass?.name)
    }

    /** Closes this vault and waits for the work it had accepted to finish. */
    private fun CredentialVault.awaitClosedAfterClose(): Boolean {
        close()
        return awaitClosed(TimeUnit.SECONDS.toMillis(WAIT_SECONDS))
    }

    @Test
    fun aBlockingCallFromInsideADeliveryAnswersRatherThanWaitingForever() {
        val answered = CountDownLatch(1)
        val reentrant = AtomicReference<VaultOutcome<StoredCredential?>?>()
        val recipient =
            VaultRecipient<StoredCredential?> {
                reentrant.set(vault.read())
                answered.countDown()
            }

        vault.readAsync(recipient)

        assertTrue(
            "a reentrant blocking read never answered",
            answered.await(WAIT_SECONDS, TimeUnit.SECONDS),
        )
        // It ran where it stood rather than waiting for the thread it was on.
        assertNull(requireNotNull(reentrant.get()).completed())
    }

    @Test
    fun waitingForACloseFromInsideADeliveryDoesNotWaitForItself() {
        val answered = CountDownLatch(1)
        val waited = AtomicReference<Boolean?>()
        val recipient =
            VaultRecipient<StoredCredential?> {
                vault.close()
                waited.set(vault.awaitClosed(TimeUnit.SECONDS.toMillis(WAIT_SECONDS)))
                answered.countDown()
            }

        vault.readAsync(recipient)

        assertTrue(
            "a reentrant wait never returned",
            answered.await(REENTRANT_WAIT_SECONDS, TimeUnit.SECONDS),
        )
        assertEquals(false, waited.get())
    }

    @Test
    fun newWorkFromInsideADeliveryIsRefusedOnceThatDeliveryHasClosedTheVault() {
        val first = StoredCredential(syntheticToken("first"))
        vault.replace(first).completed()
        val stored = requireNotNull(file.record).copyOf()
        val attempts = file.attempts
        val delivered = CountDownLatch(1)
        val afterClose = AtomicReference<VaultOutcome<Unit>?>()

        vault.readAsync(
            VaultRecipient {
                vault.close()
                afterClose.set(vault.replace(StoredCredential(syntheticToken("second"))))
                delivered.countDown()
            },
        )

        assertTrue(delivered.await(WAIT_SECONDS, TimeUnit.SECONDS))
        assertEquals(VaultFailure.VaultClosed, requireNotNull(afterClose.get()).refusal())
        assertArrayEquals("a closed vault wrote", stored, file.record)
        assertEquals(
            "a closed vault attempted a replacement",
            attempts.toLong(),
            file.attempts.toLong(),
        )
    }

    @Test
    fun workAcceptedBeforeADeliveryClosedTheVaultStillFinishes() {
        val submitted = CountDownLatch(1)
        val delivered = CountDownLatch(1)
        val answer = AtomicReference<VaultOutcome<Unit>?>()

        vault.readAsync(
            VaultRecipient {
                submitted.await(WAIT_SECONDS, TimeUnit.SECONDS)
                vault.close()
            },
        )
        vault.replaceAsync(
            StoredCredential(syntheticToken("second")),
            VaultRecipient {
                answer.set(it)
                delivered.countDown()
            },
        )
        submitted.countDown()

        assertTrue(delivered.await(WAIT_SECONDS, TimeUnit.SECONDS))
        requireNotNull(answer.get()).completed()
        assertEquals(1L, file.attempts.toLong())
    }

    @Test
    fun anObserverThatThrowsWhileReportingAFaultIsContainedOnEveryRoute() {
        captureUncaughtFaults()
        val thrown = IllegalStateException::class.java.name
        val throwing = FakeDeliveryFaults { IllegalStateException(QUOTED_INPUT) }
        val observed =
            vaults.open(keys = FakeVaultKeys(), file = FakeRecordFile(), faults = throwing)
                .completed()
        val delivered = CountDownLatch(1)

        observed.readAsync(
            VaultRecipient {
                delivered.countDown()
                throw IllegalStateException(QUOTED_INPUT)
            },
        )
        assertTrue(delivered.await(WAIT_SECONDS, TimeUnit.SECONDS))
        assertTrue(observed.awaitClosedAfterClose())

        // A refused route answers where it was asked, so an observer that throws there
        // throws at this caller.
        val refused =
            VaultRecipient<StoredCredential?> { throw IllegalStateException(QUOTED_INPUT) }
        answered { observed.readAsync(refused) }

        assertEquals(listOf(thrown, thrown), throwing.origins)
        assertNull("a fault of the observer reached the thread", uncaught.get()?.javaClass?.name)
    }

    private fun captureUncaughtFaults() {
        previous = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { _, error -> uncaught.set(error) }
    }

    private companion object {

        /** Every synthetic token a test makes begins with this. */
        const val MARKER = "synthetic-"

        const val WAIT_SECONDS = 5L

        /** How long a delivery stays inside its callback, for disposal to meet it. */
        const val HOLD_MILLIS = 1_000L

        /** Longer than the wait inside the delivery, so a self-block shows up. */
        const val REENTRANT_WAIT_SECONDS = 15L
    }
}
