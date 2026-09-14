/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth.renewal

import io.github.b_vitamins.slipbox.auth.GithubEndpoint
import io.github.b_vitamins.slipbox.auth.RecordedTransport
import io.github.b_vitamins.slipbox.auth.errorBody
import io.github.b_vitamins.slipbox.auth.ok
import io.github.b_vitamins.slipbox.auth.refused
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class RenewalScopeTest {

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
    fun aSecondAccountNeitherLendsNorTakesThisScopesCredential() {
        val other = scopeOf(testRequest(accountId = OTHER_ACCOUNT_ID))
        val theirs = keptCredential()
        storage.keep(other, theirs)
        transport.answers(GithubEndpoint.ACCESS_TOKEN, ok(rotatedBody()))

        assertEquals(
            RenewalOutcome.Reauthorize(ReauthorizationReason.NoStoredCredential),
            owner.credential(),
        )

        storage.keep(scope, keptCredential(expiresAt = clock.epoch))
        assertEquals(RenewalOutcome.Renewed(rotatedCredential(clock.epoch)), owner.credential())
        assertEquals(theirs, storage.stored(other))
    }

    @Test
    fun disconnectRemovesTheCredentialAndKeepsWhatIsReadableOffline() {
        storage.keep(scope, keptCredential())
        val cached = storage.cached(scope)

        assertEquals(RenewalRemoval.Removed, owner.disconnect())

        assertNull(storage.file(scope).record)
        assertTrue(storage.keys.aliases.toString(), storage.keys.aliases.isEmpty())
        assertTrue(cached.path, cached.exists())
        assertEquals(
            RenewalOutcome.Reauthorize(ReauthorizationReason.NoStoredCredential),
            owner.credential(),
        )
        assertEquals(0, transport.exchangesOf(GithubEndpoint.ACCESS_TOKEN))
    }

    @Test
    fun removesTheOfflineDataOnlyWhereThatRemovalIsAskedFor() {
        val kept = keptCredential()
        storage.keep(scope, kept)
        val cached = storage.cached(scope)

        assertEquals(RenewalRemoval.Removed, owner.removeCache())

        assertFalse(cached.path, cached.exists())
        assertEquals(RenewalOutcome.Current(kept), owner.credential())
    }

    @Test
    fun keepsTheCredentialStoreAndTheOfflineDataWhenARefreshIsRejected() {
        val kept = keptCredential(expiresAt = clock.epoch)
        storage.keep(scope, kept)
        val cached = storage.cached(scope)
        transport.answers(GithubEndpoint.ACCESS_TOKEN, refused(errorBody("bad_refresh_token")))

        val reauthorize = RenewalOutcome.Reauthorize(ReauthorizationReason.RefreshRejected)
        assertEquals(reauthorize, owner.credential())
        assertEquals(reauthorize, owner.credential())

        assertEquals(1, transport.exchangesOf(GithubEndpoint.ACCESS_TOKEN))
        assertEquals(kept, storage.stored(scope))
        assertTrue(cached.path, cached.exists())
        assertFalse(storage.keys.aliases.isEmpty())
    }

    private companion object {

        const val OTHER_ACCOUNT_ID = "42000002"
    }
}
