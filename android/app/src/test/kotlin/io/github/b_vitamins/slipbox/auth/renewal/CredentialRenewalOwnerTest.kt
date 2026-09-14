/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth.renewal

import io.github.b_vitamins.slipbox.auth.AuthorizationReply
import io.github.b_vitamins.slipbox.auth.GithubEndpoint
import io.github.b_vitamins.slipbox.auth.RecordedTransport
import io.github.b_vitamins.slipbox.auth.ok
import io.github.b_vitamins.slipbox.security.FakeForegroundThread
import io.github.b_vitamins.slipbox.security.VaultFailure
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class CredentialRenewalOwnerTest {

    @get:Rule
    val temporary = TemporaryFolder()

    private val transport = RecordedTransport()
    private val clock = MovingClock()
    private val request = testRequest()
    private val scope = scopeOf(request)
    private val storage by lazy { TestRenewalStorage(temporary.newFolder("private")) }
    private val owner by lazy { testOwner(storage, transport, clock, request) }

    @After
    fun releaseTheRecords() {
        storage.closeAll()
    }

    @Test
    fun usesAnUnexpiredCredentialWithoutExchangingIt() {
        val kept = keptCredential()
        storage.keep(scope, kept)

        assertEquals(RenewalOutcome.Current(kept), owner.credential())
        assertEquals(0, transport.exchangesOf(GithubEndpoint.ACCESS_TOKEN))
    }

    @Test
    fun renewsASpentCredentialAndCommitsItBeforeAnsweringWithIt() {
        storage.keep(scope, keptCredential(expiresAt = clock.epoch))
        transport.answers(GithubEndpoint.ACCESS_TOKEN, ok(rotatedBody()))
        val rotated = rotatedCredential(clock.epoch)

        assertEquals(RenewalOutcome.Renewed(rotated), owner.credential())

        assertEquals(rotated, storage.stored(scope))
        assertEquals(1, transport.exchangesOf(GithubEndpoint.ACCESS_TOKEN))
    }

    @Test
    fun renewsACredentialTooNearItsExpiryToSend() {
        storage.keep(scope, keptCredential(expiresAt = clock.epoch + WITHIN_MARGIN_SECONDS))
        transport.answers(GithubEndpoint.ACCESS_TOKEN, ok(rotatedBody()))

        assertEquals(RenewalOutcome.Renewed(rotatedCredential(clock.epoch)), owner.credential())
    }

    @Test
    fun datesTheNewExpiryAtTheAnswerAndNotAtItsStorage() {
        storage.keep(scope, keptCredential(expiresAt = clock.epoch))
        transport.answers(GithubEndpoint.ACCESS_TOKEN, ok(rotatedBody()))
        val answered = clock.epoch
        storage.file(scope).beforeReplace = { clock.epoch += STORAGE_SECONDS }

        assertEquals(RenewalOutcome.Renewed(rotatedCredential(answered)), owner.credential())

        assertEquals(rotatedCredential(answered), storage.stored(scope))
    }

    @Test
    fun asksForAuthorizationWhenThisScopeStoresNothing() {
        assertEquals(
            RenewalOutcome.Reauthorize(ReauthorizationReason.NoStoredCredential),
            owner.credential(),
        )
        assertEquals(0, transport.exchangesOf(GithubEndpoint.ACCESS_TOKEN))
    }

    @Test
    fun asksForAuthorizationWithoutARefreshToken() {
        storage.keep(scope, keptCredential(expiresAt = clock.epoch, refreshToken = null))

        assertEquals(
            RenewalOutcome.Reauthorize(ReauthorizationReason.NoRefreshAuthorization),
            owner.credential(),
        )
        assertEquals(0, transport.exchangesOf(GithubEndpoint.ACCESS_TOKEN))
    }

    @Test
    fun keepsARecordItCannotReadAndAsksForAuthorization() {
        storage.keep(scope, keptCredential())
        val cached = storage.cached(scope)
        storage.keys.failure = VaultFailure.KeyInvalidated

        assertEquals(
            RenewalOutcome.Reauthorize(ReauthorizationReason.StoredCredentialUnusable),
            owner.credential(),
        )
        assertNotNull(storage.file(scope).record)
        assertTrue(cached.path, cached.exists())
    }

    @Test
    fun refusesEveryCallOnTheThreadThatDrawsTheScreen() {
        val drawing =
            testOwner(
                storage,
                transport,
                clock,
                request,
                foreground = FakeForegroundThread(current = true),
            )

        assertEquals(
            RenewalOutcome.Unavailable(RenewalFault.VaultRefused(VaultFailure.ForegroundRefused)),
            drawing.credential(),
        )
        val incomplete = RenewalRemoval.Incomplete(VaultFailure.ForegroundRefused)
        assertEquals(incomplete, drawing.disconnect())
        assertEquals(incomplete, drawing.removeCache())
    }

    @Test
    fun reportsABuildThatCarriesNoRegistration() {
        storage.keep(scope, keptCredential(expiresAt = clock.epoch))
        val unregistered = testOwner(storage, transport, clock, request, app = null)

        assertEquals(
            RenewalOutcome.Unavailable(RenewalFault.ConfigurationUnavailable),
            unregistered.credential(),
        )
        assertEquals(0, transport.exchangesOf(GithubEndpoint.ACCESS_TOKEN))
    }

    @Test
    fun keepsTheStoredCredentialWhenAnExchangeCannotReachGithub() {
        val kept = keptCredential(expiresAt = clock.epoch)
        storage.keep(scope, kept)
        transport.answers(GithubEndpoint.ACCESS_TOKEN, AuthorizationReply.Failed(FAULT_ORIGIN))

        val unavailable = RenewalOutcome.Unavailable(RenewalFault.TransportFailed(FAULT_ORIGIN))
        assertEquals(unavailable, owner.credential())
        assertEquals(unavailable, owner.credential())

        assertEquals(2, transport.exchangesOf(GithubEndpoint.ACCESS_TOKEN))
        assertEquals(kept, storage.stored(scope))
    }

    @Test
    fun deliversNothingOnceItIsClosed() {
        storage.keep(scope, keptCredential())
        owner.close()

        assertEquals(RenewalOutcome.Withdrawn, owner.credential())
        assertEquals(RenewalRemoval.Withdrawn, owner.disconnect())
        assertEquals(RenewalRemoval.Withdrawn, owner.removeCache())
        assertNotNull(storage.file(scope).record)
    }

    @Test
    fun withdrawsACredentialReadAfterClose() {
        val kept = keptCredential()
        storage.keep(scope, kept)
        storage.beforeRead = { owner.close() }

        assertEquals(RenewalOutcome.Withdrawn, owner.credential())
        assertEquals(0, transport.exchangesOf(GithubEndpoint.ACCESS_TOKEN))
        assertEquals(kept, storage.stored(scope))
    }

    @Test
    fun doesNotStartARefreshAfterCloseDuringTheVaultRead() {
        val kept = keptCredential(expiresAt = clock.epoch)
        storage.keep(scope, kept)
        transport.answers(GithubEndpoint.ACCESS_TOKEN, ok(rotatedBody()))
        storage.beforeRead = { owner.close() }

        assertEquals(RenewalOutcome.Withdrawn, owner.credential())
        assertEquals(0, transport.exchangesOf(GithubEndpoint.ACCESS_TOKEN))
        assertEquals(kept, storage.stored(scope))
    }

    private companion object {

        const val WITHIN_MARGIN_SECONDS = 30L

        const val STORAGE_SECONDS = 120L

        const val FAULT_ORIGIN = "java.net.SocketTimeoutException"
    }
}
