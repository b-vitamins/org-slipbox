/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth.renewal

import android.content.Context
import io.github.b_vitamins.slipbox.auth.AuthorizationClock
import io.github.b_vitamins.slipbox.auth.AuthorizationTransport
import io.github.b_vitamins.slipbox.auth.GithubApp
import io.github.b_vitamins.slipbox.auth.HttpsAuthorizationTransport
import io.github.b_vitamins.slipbox.auth.SystemAuthorizationClock
import io.github.b_vitamins.slipbox.security.CredentialVault
import io.github.b_vitamins.slipbox.security.ForegroundThread
import io.github.b_vitamins.slipbox.security.PrivateStoragePolicy
import io.github.b_vitamins.slipbox.security.SlipboxVault
import io.github.b_vitamins.slipbox.security.StoredCredential
import io.github.b_vitamins.slipbox.security.VaultFailure
import io.github.b_vitamins.slipbox.security.VaultOutcome
import io.github.b_vitamins.slipbox.security.VaultScope
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock

/** Scoped, blocking renewal with serialized refresh and commit. Calls must run off main. */
class CredentialRenewalOwner internal constructor(
    val request: RenewalRequest,
    private val app: GithubApp?,
    private val storage: RenewalStorage,
    private val transport: AuthorizationTransport,
    private val clock: AuthorizationClock,
    private val foreground: ForegroundThread,
) : AutoCloseable {

    private val decision = ReentrantLock()

    private val composed: VaultOutcome<VaultScope> = request.scope()

    private val closed = AtomicBoolean()

    // Never replay a refresh token after a failed or uncertain local commit.
    private var barred: Barred? = null

    fun credential(): RenewalOutcome =
        when (val ready = ready()) {
            is Ready.Refused -> RenewalOutcome.Unavailable(RenewalFault.VaultRefused(ready.failure))
            Ready.Withdrawn -> RenewalOutcome.Withdrawn
            is Ready.Open ->
                decision.withLock {
                    val outcome = renew(ready.scope)
                    if (closed.get()) RenewalOutcome.Withdrawn else outcome
                }
        }

    /** Deletes this credential and its keys, retaining offline data. */
    fun disconnect(): RenewalRemoval =
        when (val ready = ready()) {
            is Ready.Refused -> RenewalRemoval.Incomplete(ready.failure)
            Ready.Withdrawn -> RenewalRemoval.Withdrawn
            is Ready.Open -> decision.withLock { remove(ready.scope) }
        }

    /** Deletes this account's offline data only when explicitly requested. */
    fun removeCache(): RenewalRemoval =
        when (val ready = ready()) {
            is Ready.Refused -> RenewalRemoval.Incomplete(ready.failure)
            Ready.Withdrawn -> RenewalRemoval.Withdrawn
            is Ready.Open -> decision.withLock { removeAccount(ready.scope) }
        }

    override fun close() {
        closed.set(true)
    }

    private fun renew(scope: VaultScope): RenewalOutcome {
        if (closed.get()) {
            return RenewalOutcome.Withdrawn
        }
        val vault =
            when (val opened = storage.open(scope)) {
                is VaultOutcome.Failed ->
                    return RenewalOutcome.Unavailable(RenewalFault.VaultRefused(opened.failure))
                is VaultOutcome.Completed -> opened.value
            }
        return try {
            renew(vault)
        } finally {
            release(vault)
        }
    }

    private fun renew(vault: CredentialVault): RenewalOutcome {
        val stored =
            when (val read = vault.read()) {
                is VaultOutcome.Failed -> return unreadable(read.failure)
                is VaultOutcome.Completed -> read.value
            }
                ?: return RenewalOutcome.Reauthorize(ReauthorizationReason.NoStoredCredential)
        if (closed.get()) {
            return RenewalOutcome.Withdrawn
        }
        if (!isSpent(stored)) {
            return RenewalOutcome.Current(stored)
        }
        val refresh =
            stored.refreshToken
                ?: return RenewalOutcome.Reauthorize(ReauthorizationReason.NoRefreshAuthorization)
        barred?.takeIf { it.refreshToken == refresh }?.let {
            return RenewalOutcome.Reauthorize(it.reason)
        }
        val registration =
            app ?: return RenewalOutcome.Unavailable(RenewalFault.ConfigurationUnavailable)
        if (closed.get()) {
            return RenewalOutcome.Withdrawn
        }
        val rotated =
            when (
                val exchange =
                    GithubTokenRefresh(registration, transport, clock).exchange(refresh)
            ) {
                is RefreshExchange.Unavailable -> return RenewalOutcome.Unavailable(exchange.fault)
                is RefreshExchange.Refused -> {
                    barred = Barred(refresh, exchange.reason)
                    return RenewalOutcome.Reauthorize(exchange.reason)
                }
                is RefreshExchange.Rotated -> exchange.credential
            }
        when (val replaced = vault.replace(rotated)) {
            is VaultOutcome.Failed -> {
                barred = Barred(refresh, ReauthorizationReason.RotationLost)
                return RenewalOutcome.Uncommitted(replaced.failure)
            }
            is VaultOutcome.Completed -> Unit
        }
        return RenewalOutcome.Renewed(rotated)
    }

    private fun remove(scope: VaultScope): RenewalRemoval {
        if (closed.get()) {
            return RenewalRemoval.Withdrawn
        }
        val vault =
            when (val opened = storage.open(scope)) {
                is VaultOutcome.Failed -> return RenewalRemoval.Incomplete(opened.failure)
                is VaultOutcome.Completed -> opened.value
            }
        try {
            when (val removed = vault.remove()) {
                is VaultOutcome.Failed -> return RenewalRemoval.Incomplete(removed.failure)
                is VaultOutcome.Completed -> Unit
            }
        } finally {
            release(vault)
        }
        return swept { it.removeCredential(scope) }
    }

    private fun removeAccount(scope: VaultScope): RenewalRemoval {
        if (closed.get()) {
            return RenewalRemoval.Withdrawn
        }
        return swept { it.removeAccount(scope) }
    }

    private fun swept(sweep: (PrivateStoragePolicy) -> VaultOutcome<Int>): RenewalRemoval {
        val policy =
            when (val outcome = storage.policy()) {
                is VaultOutcome.Failed -> return RenewalRemoval.Incomplete(outcome.failure)
                is VaultOutcome.Completed -> outcome.value
            }
        return when (val removed = sweep(policy)) {
            is VaultOutcome.Failed -> RenewalRemoval.Incomplete(removed.failure)
            is VaultOutcome.Completed -> RenewalRemoval.Removed
        }
    }

    private fun unreadable(failure: VaultFailure): RenewalOutcome =
        if (failure.reauthorize) {
            RenewalOutcome.Reauthorize(ReauthorizationReason.StoredCredentialUnusable)
        } else {
            RenewalOutcome.Unavailable(RenewalFault.VaultRefused(failure))
        }

    private fun isSpent(credential: StoredCredential): Boolean {
        val expiresAt = credential.expiresAtEpochSeconds ?: return false
        return clock.epochSeconds() + EXPIRY_MARGIN_SECONDS >= expiresAt
    }

    private fun ready(): Ready =
        when {
            foreground.isCurrent() -> Ready.Refused(VaultFailure.ForegroundRefused)
            closed.get() -> Ready.Withdrawn
            else ->
                when (val scope = composed) {
                    is VaultOutcome.Failed -> Ready.Refused(scope.failure)
                    is VaultOutcome.Completed -> Ready.Open(scope.value)
                }
        }

    private fun release(vault: CredentialVault) {
        vault.close()
        vault.awaitClosed(RELEASE_TIMEOUT_MILLIS)
    }

    private class Barred(val refreshToken: String, val reason: ReauthorizationReason)

    private sealed interface Ready {

        class Open(val scope: VaultScope) : Ready

        class Refused(val failure: VaultFailure) : Ready

        object Withdrawn : Ready
    }

    companion object {

        fun packaged(context: Context, request: RenewalRequest): CredentialRenewalOwner =
            CredentialRenewalOwner(
                request = request,
                app = GithubApp.packaged(),
                storage = SlipboxRenewalStorage(context),
                transport = HttpsAuthorizationTransport(),
                clock = SystemAuthorizationClock,
                foreground = SlipboxVault.MainThread,
            )

        private const val EXPIRY_MARGIN_SECONDS = 60L

        private const val RELEASE_TIMEOUT_MILLIS = 10_000L
    }
}
