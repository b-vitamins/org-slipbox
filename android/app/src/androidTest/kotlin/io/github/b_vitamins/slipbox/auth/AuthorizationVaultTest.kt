/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth

import android.util.Log
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import io.github.b_vitamins.slipbox.security.CredentialVault
import io.github.b_vitamins.slipbox.security.SlipboxVault
import io.github.b_vitamins.slipbox.security.TOKEN_MARKER
import io.github.b_vitamins.slipbox.security.VaultFailure
import io.github.b_vitamins.slipbox.security.VaultNamespace
import io.github.b_vitamins.slipbox.security.VaultOutcome
import io.github.b_vitamins.slipbox.security.VaultProbeRun
import io.github.b_vitamins.slipbox.security.VaultReauthorization
import io.github.b_vitamins.slipbox.security.VaultRecipient
import io.github.b_vitamins.slipbox.security.VaultScope
import io.github.b_vitamins.slipbox.security.completed
import io.github.b_vitamins.slipbox.security.refusal
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNotSame
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class AuthorizationVaultTest {

    private val context = InstrumentationRegistry.getInstrumentation().targetContext

    private val run = VaultProbeRun(context)

    private val kept = mutableListOf<VaultScope>()

    @After
    fun releaseThisRun() {
        for (scope in kept) {
            run.policy.removeCredential(scope).completed()
            run.policy.removeAccount(scope).completed()
        }
        val remains = run.release()
        assertEquals(remains.aliases.toString(), emptyList<String>(), remains.aliases)
        assertFalse(run.policy.root.path, remains.rootExists)
    }

    @Test
    fun theScopeIsGitHubsAuthorityAndTheAccountGitHubConfirmed() {
        val authorization = syntheticAuthorization()

        val scope = scopeOf(authorization)

        assertEquals(GithubApp.PROVIDER_AUTHORITY, scope.providerAuthority)
        assertEquals(PROBE_ACCOUNT_ID, scope.accountId)
        assertEquals(SOURCE_ID, scope.sourceId)
        assertEquals(CREDENTIAL_REF, scope.credentialRef)
        assertEquals(
            VaultScope.of(SOURCE_ID, GithubApp.PROVIDER_AUTHORITY, PROBE_ACCOUNT_ID, CREDENTIAL_REF)
                .completed(),
            scope,
        )
        assertNotEquals(
            "a name a reader could type named the record",
            authorization.account.login,
            scope.accountId,
        )
    }

    @Test
    fun aKeptGrantIsSealedUnderAFreshKeyAndReadsBackAsItWasIssued() {
        val authorization = syntheticAuthorization()
        val scope = scopeOf(authorization)
        val answered = CountDownLatch(1)
        val answer = AtomicReference<VaultOutcome<VaultReauthorization>?>(null)
        val answeredOn = AtomicReference<Thread?>(null)

        AuthorizationVault(context, run.namespace).keep(
            sourceId = SOURCE_ID,
            credentialRef = CREDENTIAL_REF,
            authorization = authorization,
            recipient =
                VaultRecipient { outcome ->
                    answeredOn.set(Thread.currentThread())
                    answer.set(outcome)
                    answered.countDown()
                },
        )

        assertTrue(
            "no answer reached the recipient",
            answered.await(ANSWER_MILLIS, TimeUnit.MILLISECONDS),
        )
        assertNotSame(
            "the vault answered on the thread that asked, which a screen may not do",
            Thread.currentThread(),
            answeredOn.get(),
        )
        val reauthorized = requireNotNull(answer.get()).completed()
        assertEquals(VaultNamespace.FIRST_GENERATION, reauthorized.generation)
        assertEquals("a first authorization superseded a key", 0, reauthorized.supersededKeys)
        assertNull(reauthorized.cleanup)
        assertTrue(
            "the fresh key of this grant is not in the keystore",
            run.aliases().contains(run.alias(scope)),
        )

        val record = run.record(scope)
        assertTrue(record.path, record.isFile)
        val sealed = record.readBytes()
        assertFalse(
            "the record holds its token as it was given",
            String(sealed, Charsets.ISO_8859_1).contains(TOKEN_MARKER),
        )

        assertEquals(authorization.credential, readAndClose(reopened(scope)))
        Log.i(TAG, "a grant was sealed in ${sealed.size} bytes and read back by its own scope")
    }

    @Test
    fun anIdentityThatCannotNameARecordIsRefusedToTheCallerAndNothingIsOpened() {
        val authorization = syntheticAuthorization()
        val answered = CountDownLatch(1)
        val answer = AtomicReference<VaultOutcome<VaultReauthorization>?>(null)

        AuthorizationVault(context, run.namespace).keep(
            sourceId = "s".repeat(VaultScope.MAX_FIELD_LENGTH + 1),
            credentialRef = CREDENTIAL_REF,
            authorization = authorization,
            recipient =
                VaultRecipient { outcome ->
                    answer.set(outcome)
                    answered.countDown()
                },
        )

        assertTrue(
            "a refusal never reached the caller",
            answered.await(ANSWER_MILLIS, TimeUnit.MILLISECONDS),
        )
        val refusal = requireNotNull(answer.get()).refusal()
        assertFalse(refusal.toString(), refusal.toString().contains(TOKEN_MARKER))
        assertEquals("a refused identity made a key anyway", emptyList<String>(), run.aliases())
    }


    private fun scopeOf(authorization: GithubAuthorization): VaultScope =
        AuthorizationVault(context, run.namespace)
            .scopeFor(SOURCE_ID, CREDENTIAL_REF, authorization.account)
            .completed()
            .also { if (!kept.contains(it)) kept.add(it) }


    private fun readAndClose(vault: CredentialVault) =
        try {
            vault.read().completed()
        } finally {
            vault.close()
            assertTrue(vault.awaitClosed(CLOSE_MILLIS))
        }


    private fun reopened(scope: VaultScope): CredentialVault {
        val deadline = System.currentTimeMillis() + CLOSE_MILLIS
        while (true) {
            when (val outcome = SlipboxVault.open(context, run.namespace, scope)) {
                is VaultOutcome.Completed -> return outcome.value
                is VaultOutcome.Failed -> {
                    assertEquals(VaultFailure.VaultHeld, outcome.failure)
                    assertTrue(
                        "the record of a kept grant was never released",
                        System.currentTimeMillis() < deadline,
                    )
                    Thread.sleep(RELEASE_POLL_MILLIS)
                }
            }
        }
    }

    private companion object {

        const val TAG = "SlipboxAuthProbe"


        const val SOURCE_ID = "probe-source-github-1"

        const val CREDENTIAL_REF = "probe-credential-1"


        const val CLOSE_MILLIS = 10_000L

        const val RELEASE_POLL_MILLIS = 25L
    }
}
