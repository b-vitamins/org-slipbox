/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth.renewal

import io.github.b_vitamins.slipbox.auth.GithubEndpoint
import io.github.b_vitamins.slipbox.auth.RecordedTransport
import io.github.b_vitamins.slipbox.auth.WAIT_MILLIS
import io.github.b_vitamins.slipbox.auth.ok
import io.github.b_vitamins.slipbox.security.CommitPhase
import io.github.b_vitamins.slipbox.security.StoredCredential
import io.github.b_vitamins.slipbox.security.VaultFailure
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

class RenewalRotationTest {

    @get:Rule
    val temporary = TemporaryFolder()

    private val transport = RecordedTransport()
    private val clock = MovingClock()
    private val request = testRequest()
    private val scope = scopeOf(request)
    private val exchanging = CountDownLatch(1)
    private val release = CountDownLatch(1)
    private val storage by lazy { TestRenewalStorage(temporary.newFolder("private")) }
    private val owner by lazy { testOwner(storage, transport, clock, request) }

    @After
    fun releaseTheRecords() {
        release.countDown()
        storage.closeAll()
    }

    @Test
    fun spendsARotatingRefreshTokenOnceForCallersThatArriveTogether() {
        val rotated = rotatedCredential(clock.epoch)
        spentCredential()
        holdTheExchange()

        val first = Answering { owner.credential() }.start()
        awaitTheExchange()
        val second = Answering { owner.credential() }.start()
        second.awaitQueued()
        release.countDown()

        assertEquals(RenewalOutcome.Renewed(rotated), first.answer())
        assertEquals(RenewalOutcome.Current(rotated), second.answer())
        assertEquals(1, transport.exchangesOf(GithubEndpoint.ACCESS_TOKEN))
        assertEquals(rotated, storage.stored(scope))
    }

    @Test
    fun refusesASecondOwnerWhileTheFirstHoldsTheRecord() {
        val rotated = rotatedCredential(clock.epoch)
        spentCredential()
        holdTheExchange()
        val other = testOwner(storage, RecordedTransport(), clock, request)

        val first = Answering { owner.credential() }.start()
        awaitTheExchange()

        assertEquals(
            RenewalOutcome.Unavailable(RenewalFault.VaultRefused(VaultFailure.VaultHeld)),
            other.credential(),
        )
        release.countDown()
        assertEquals(RenewalOutcome.Renewed(rotated), first.answer())
        assertEquals(1, transport.exchangesOf(GithubEndpoint.ACCESS_TOKEN))
    }

    @Test
    fun reportsARotationItCouldNotStoreAndDoesNotSpendItAgain() {
        val kept = spentCredential()
        val cached = storage.cached(scope)
        val failure = VaultFailure.WriteFailed(CommitPhase.Unknown, FAULT_ORIGIN)
        storage.file(scope).writeFailure = failure

        assertEquals(RenewalOutcome.Uncommitted(failure), owner.credential())
        assertEquals(
            RenewalOutcome.Reauthorize(ReauthorizationReason.RotationLost),
            owner.credential(),
        )

        assertEquals(1, transport.exchangesOf(GithubEndpoint.ACCESS_TOKEN))
        assertEquals(kept, storage.stored(scope))
        assertTrue(cached.path, cached.exists())
    }

    @Test
    fun deliversTheCredentialAnUnreportedCommitTurnedOutToHaveStored() {
        val rotated = rotatedCredential(clock.epoch)
        spentCredential()
        val unreported = VaultFailure.WriteFailed(CommitPhase.Unknown, FAULT_ORIGIN)
        storage.file(scope).writeFailure = unreported

        assertEquals(RenewalOutcome.Uncommitted(unreported), owner.credential())

        storage.file(scope).writeFailure = null
        storage.keep(scope, rotated)

        assertEquals(RenewalOutcome.Current(rotated), owner.credential())
        assertEquals(1, transport.exchangesOf(GithubEndpoint.ACCESS_TOKEN))
    }

    @Test
    fun withdrawsARotationThatArrivedAfterItWasClosed() {
        val rotated = rotatedCredential(clock.epoch)
        spentCredential()
        transport.beforeExchange = { owner.close() }

        assertEquals(RenewalOutcome.Withdrawn, owner.credential())

        assertEquals(rotated, storage.stored(scope))
        val successor = testOwner(storage, RecordedTransport(), clock, request)
        assertEquals(RenewalOutcome.Current(rotated), successor.credential())
    }

    private fun spentCredential(): StoredCredential {
        val kept = keptCredential(expiresAt = clock.epoch)
        storage.keep(scope, kept)
        transport.answers(GithubEndpoint.ACCESS_TOKEN, ok(rotatedBody()))
        return kept
    }

    private fun holdTheExchange() {
        transport.beforeExchange = {
            exchanging.countDown()
            release.await(WAIT_MILLIS, TimeUnit.MILLISECONDS)
        }
    }

    private fun awaitTheExchange() {
        assertTrue(exchanging.await(WAIT_MILLIS, TimeUnit.MILLISECONDS))
    }

    private companion object {

        const val FAULT_ORIGIN = "java.io.IOException"
    }
}
