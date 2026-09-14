/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth

import java.util.concurrent.CountDownLatch
import java.util.concurrent.atomic.AtomicReference
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

class GithubAuthorizationOwnerTest {

    private val clock = SteppedClock()

    private val delivery = QueuedDelivery()

    private val owners = mutableListOf<GithubAuthorizationOwner>()

    private val opened = mutableListOf<CountDownLatch>()

    @After
    fun releaseTheOwners() {
        opened.forEach { it.countDown() }
        owners.forEach { it.close() }
    }

    @Test
    fun aBuildWithNoRegistrationSaysSoWithoutContactingGitHub() {
        val transport = RecordedTransport()
        val owner = ownerOf(app = null, transport = transport)
        val listener = RecordingListener()

        val attempt = owner.authorize(listener)
        assertTrue(delivery.awaitPosts(1))
        delivery.drain()

        assertFalse(owner.isConfigured)
        assertEquals(
            AuthorizationOutcome.Unavailable(AuthorizationFault.ConfigurationUnavailable),
            listener.outcome,
        )
        assertEquals("GitHub was contacted anyway", 0, transport.requests.size)
        assertFalse(attempt.isLive)
        assertNull(owner.running())
    }

    @Test
    fun theLatestAttemptIsTheRunningOneAndNoTwoAreTheSame() {
        val owner = ownerOf(transport = grantingTransport())

        val first = owner.authorize(RecordingListener())
        val second = owner.authorize(RecordingListener())

        assertNotEquals(first.serial, second.serial)
        assertSame(second, owner.running())
        assertFalse(first.isLive)
    }

    @Test
    fun aReplacedAttemptStopsPollingAndDeliversNothing() {
        val gate = held()
        val transport = grantingTransport(polls = listOf(pending(), ok(grantBody())))
        val owner = ownerOf(transport = transport)
        val abandoned = RecordingListener()

        val first = owner.authorize(abandoned)
        assertTrue(delivery.awaitPosts(1))
        delivery.drain()
        assertEquals("the code was never shown", 1, abandoned.waiting.size)

        clock.pause = null
        val kept = RecordingListener()
        owner.authorize(kept)

        assertFalse(first.isLive)
        assertTrue(delivery.awaitPosts(POSTS_OF_TWO_ATTEMPTS))
        delivery.drain()
        assertEquals(
            "a replaced attempt reported an outcome",
            emptyList<AuthorizationOutcome>(),
            abandoned.settled,
        )
        kept.outcome.authorized()
        gate.countDown()
    }

    @Test
    fun aGrantThatArrivesForAReplacedAttemptIsDroppedRatherThanDelivered() {
        val transport = grantingTransport()
        val owner = ownerOf(transport = transport)
        val abandoned = RecordingListener()

        val first = owner.authorize(abandoned)
        assertTrue(delivery.awaitPosts(2))
        val kept = RecordingListener()
        owner.authorize(kept)
        assertFalse(first.isLive)

        assertTrue(delivery.awaitPosts(POSTS_OF_TWO_ATTEMPTS))
        delivery.drain()

        assertEquals(
            "an old grant activated a replaced attempt",
            emptyList<AuthorizationOutcome>(),
            abandoned.settled,
        )
        assertEquals(
            "a replaced attempt still showed a code",
            emptyList<DeviceGrant>(),
            abandoned.waiting,
        )
        kept.outcome.authorized()
    }

    @Test
    fun cancellationWithdrawsBothThePollingAndTheDelivery() {
        val gate = held()
        val transport = grantingTransport(polls = listOf(pending()))
        val owner = ownerOf(transport = transport)
        val listener = RecordingListener()

        val attempt = owner.authorize(listener)
        assertTrue(delivery.awaitPosts(1))
        delivery.drain()
        assertEquals(1, listener.waiting.size)

        attempt.cancel()

        assertFalse(attempt.isLive)
        assertNull(owner.running())
        assertTrue(delivery.awaitPosts(2))
        delivery.drain()
        assertEquals(
            "a cancelled attempt reported an outcome",
            emptyList<AuthorizationOutcome>(),
            listener.settled,
        )
        assertEquals(
            "a cancelled attempt kept polling",
            0,
            transport.exchangesOf(GithubEndpoint.ACCESS_TOKEN),
        )
        gate.countDown()
    }

    @Test
    fun closingTheOwnerWithdrawsTheAttemptAndStartsNoOther() {
        val gate = held()
        val transport = grantingTransport(polls = listOf(pending()))
        val owner = ownerOf(transport = transport)
        val running = RecordingListener()
        val attempt = owner.authorize(running)
        assertTrue(delivery.awaitPosts(1))

        owner.close()

        assertFalse(attempt.isLive)
        assertNull(owner.running())
        val later = RecordingListener()
        val refused = owner.authorize(later)
        assertFalse(refused.isLive)
        delivery.drain()
        assertEquals(emptyList<AuthorizationOutcome>(), running.settled)
        assertEquals(emptyList<AuthorizationOutcome>(), later.settled)
        assertEquals("a closed owner requested a code", 1, transport.requests.size)
        gate.countDown()
    }

    @Test
    fun nothingIsDeliveredOnTheAttemptsOwnThread() {
        val seen = AtomicReference<Thread?>()
        val listener =
            object : AuthorizationListener {

                override fun onVerificationWaiting(grant: DeviceGrant) {
                    seen.set(Thread.currentThread())
                }

                override fun onSettled(outcome: AuthorizationOutcome) {
                    seen.set(Thread.currentThread())
                }
            }
        val owner = ownerOf(transport = grantingTransport())

        owner.authorize(listener)
        assertTrue(delivery.awaitPosts(2))
        delivery.drain()

        assertSame("delivery left the thread it was given", Thread.currentThread(), seen.get())
    }

    @Test
    fun anAttemptThatFailsOutrightReportsAFaultOfItsOwn() {
        val transport = AuthorizationTransport { throw IllegalStateException(SYNTHETIC) }
        val owner = ownerOf(transport = transport)
        val listener = RecordingListener()

        owner.authorize(listener)
        assertTrue(delivery.awaitPosts(1))
        delivery.drain()

        val fault = listener.outcome.unavailable()
        assertEquals(AuthorizationFault.AttemptFailed(ILLEGAL_STATE), fault)
        assertEquals(AuthorizationStage.Attempt, fault.stage)
        assertFalse("the platform's own wording escaped", fault.toString().contains(SYNTHETIC))
    }

    @Test
    fun anAttemptPrintsWhichOneItIsAndNothingOfTheAccounts() {
        val owner = ownerOf(transport = grantingTransport())

        val attempt = owner.authorize(RecordingListener())

        assertEquals("AuthorizationAttempt(${attempt.serial}, Running)", attempt.toString())
    }


    private fun ownerOf(
        app: GithubApp? = testApp(),
        transport: AuthorizationTransport,
    ): GithubAuthorizationOwner =
        GithubAuthorizationOwner(app, transport, clock, delivery).also { owners.add(it) }


    private fun held(): CountDownLatch =
        CountDownLatch(1).also {
            clock.pause = it
            opened.add(it)
        }

    private companion object {


        const val POSTS_OF_TWO_ATTEMPTS = 4

        const val SYNTHETIC = "a synthetic platform fault"

        const val ILLEGAL_STATE = "java.lang.IllegalStateException"
    }
}
