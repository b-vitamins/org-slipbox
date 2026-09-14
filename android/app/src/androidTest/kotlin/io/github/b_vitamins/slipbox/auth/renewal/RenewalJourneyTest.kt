/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth.renewal

import android.util.Log
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import io.github.b_vitamins.slipbox.auth.ANSWER_MILLIS
import io.github.b_vitamins.slipbox.auth.AuthorizationVault
import io.github.b_vitamins.slipbox.auth.DeviceEndpoint
import io.github.b_vitamins.slipbox.auth.EPOCH_SECONDS
import io.github.b_vitamins.slipbox.auth.GithubApp
import io.github.b_vitamins.slipbox.auth.INSTALLATION_PAGE
import io.github.b_vitamins.slipbox.auth.PROBE_CLIENT_ID
import io.github.b_vitamins.slipbox.auth.TOKEN_LIFETIME_SECONDS
import io.github.b_vitamins.slipbox.security.CredentialVault
import io.github.b_vitamins.slipbox.security.PrivateStore
import io.github.b_vitamins.slipbox.security.SlipboxVault
import io.github.b_vitamins.slipbox.security.StoredCredential
import io.github.b_vitamins.slipbox.security.TOKEN_MARKER
import io.github.b_vitamins.slipbox.security.VaultFailure
import io.github.b_vitamins.slipbox.security.VaultNamespace
import io.github.b_vitamins.slipbox.security.VaultOutcome
import io.github.b_vitamins.slipbox.security.VaultProbeRun
import io.github.b_vitamins.slipbox.security.VaultReauthorization
import io.github.b_vitamins.slipbox.security.VaultRecipient
import io.github.b_vitamins.slipbox.security.completed
import io.github.b_vitamins.slipbox.security.syntheticToken
import java.io.File
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class RenewalJourneyTest {

    private val context = InstrumentationRegistry.getInstrumentation().targetContext

    private val run = VaultProbeRun(context)

    private val authorization = spentAuthorization()

    private val scope =
        AuthorizationVault(context, run.namespace)
            .scopeFor(SOURCE_ID, CREDENTIAL_REF, authorization.account)
            .completed()

    private val rotatedAccess = syntheticToken("rotated-access")

    private val rotatedRefresh = syntheticToken("rotated-refresh")

    private val transport = RotatingTransport(rotatedAccess, rotatedRefresh)

    @After
    fun releaseThisRun() {
        run.policy.removeCredential(scope).completed()
        run.policy.removeAccount(scope).completed()
        val remains = run.release()
        assertEquals(remains.aliases.toString(), emptyList<String>(), remains.aliases)
        assertFalse(run.policy.root.path, remains.rootExists)
    }

    @Test
    fun aSpentGrantIsRenewedInPlaceAndDisconnectLeavesTheReadingCache() {
        keepTheGrant()
        val stored = readAndClose(reopened())
        assertEquals(authorization.credential, stored)
        val cached = cachedReadingState()
        val record = run.record(scope)
        val alias = run.alias(scope)
        assertTrue(record.path, record.isFile)
        val renewed =
            StoredCredential(rotatedAccess, rotatedRefresh, EPOCH_SECONDS + TOKEN_LIFETIME_SECONDS)
        val owner = probeOwner()

        assertEquals(RenewalOutcome.Renewed(renewed), owner.credential())

        val sent = transport.requests.single()
        assertEquals(DeviceEndpoint.ACCESS_TOKEN, sent.url)
        assertTrue(
            "the exchange did not send the stored refresh token",
            sent.form?.get("refresh_token") == requireNotNull(stored).refreshToken,
        )
        assertFalse(sent.toString(), sent.toString().contains(TOKEN_MARKER))
        val sealed = record.readBytes()
        assertFalse(
            "the renewed record holds a token as it was issued",
            String(sealed, Charsets.ISO_8859_1).contains(TOKEN_MARKER),
        )
        assertEquals(
            "renewal provisioned a key instead of keeping this scope's",
            listOf(alias),
            run.aliases(),
        )
        assertEquals(RenewalOutcome.Current(renewed), probeOwner().credential())
        assertEquals("a second caller exchanged again", 1, transport.requests.size)

        assertEquals(RenewalRemoval.Removed, owner.disconnect())

        assertFalse(record.path, record.exists())
        assertEquals(emptyList<String>(), run.aliases())
        assertTrue(cached.path, cached.exists())
        Log.i(TAG, "a spent grant renewed into ${sealed.size} sealed bytes and disconnected")
    }

    private fun probeOwner(): CredentialRenewalOwner =
        CredentialRenewalOwner(
            request = RenewalRequest(SOURCE_ID, authorization.account, CREDENTIAL_REF),
            app = requireNotNull(GithubApp.of(PROBE_CLIENT_ID, INSTALLATION_PAGE)),
            storage = SlipboxRenewalStorage(context, run.namespace),
            transport = transport,
            clock = SettledClock,
            foreground = SlipboxVault.MainThread,
        )

    private fun keepTheGrant() {
        val answered = CountDownLatch(1)
        val answer = AtomicReference<VaultOutcome<VaultReauthorization>?>(null)

        AuthorizationVault(context, run.namespace).keep(
            sourceId = SOURCE_ID,
            credentialRef = CREDENTIAL_REF,
            authorization = authorization,
            recipient =
                VaultRecipient { outcome ->
                    answer.set(outcome)
                    answered.countDown()
                },
        )

        assertTrue(
            "no answer reached the recipient",
            answered.await(ANSWER_MILLIS, TimeUnit.MILLISECONDS),
        )
        val kept = requireNotNull(answer.get()).completed()
        assertEquals(VaultNamespace.FIRST_GENERATION, kept.generation)
    }

    private fun cachedReadingState(): File {
        val file = run.policy.fileFor(PrivateStore.Reading, scope, READING_NAME).completed()
        requireNotNull(file.parentFile).mkdirs()
        file.writeBytes(READING_BYTES)
        return file
    }

    private fun readAndClose(vault: CredentialVault): StoredCredential? =
        try {
            vault.read().completed()
        } finally {
            vault.close()
            assertTrue(vault.awaitClosed(CLOSE_MILLIS))
        }

    private fun reopened(): CredentialVault {
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

        const val TAG = "SlipboxRenewalProbe"

        const val SOURCE_ID = "probe-source-github-1"

        const val CREDENTIAL_REF = "probe-credential-1"

        const val READING_NAME = "reading.bin"

        val READING_BYTES = "probe-reading-state".toByteArray()

        const val CLOSE_MILLIS = 10_000L

        const val RELEASE_POLL_MILLIS = 25L
    }
}
