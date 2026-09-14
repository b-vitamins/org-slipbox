/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import android.util.Log
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

@RunWith(AndroidJUnit4::class)
class VaultHandoffTest {

    private val context = InstrumentationRegistry.getInstrumentation().targetContext

    private val run = VaultProbeRun(context)

    private val faults = ProbeDeliveryFaults()

    @After
    fun releaseThisRun() {
        val remains = run.release()
        assertEquals(remains.aliases.toString(), emptyList<String>(), remains.aliases)
        assertFalse(run.policy.root.path, remains.rootExists)
    }

    @Test
    fun aRecipientThatTakesTheVaultGetsOneThatStoresAndReadsBack() {
        val scope = run.scope()
        val credential = StoredCredential(syntheticToken("access"))
        val answered = CountDownLatch(1)
        val answer = AtomicReference<VaultOutcome<CredentialVault>?>(null)
        val recipient =
            VaultRecipient<CredentialVault> { outcome ->
                answer.set(outcome)
                answered.countDown()
            }

        SlipboxVault.openAsync(context, run.namespace, scope, recipient, faults)

        assertTrue(answered.await(ANSWER_TIMEOUT_MILLIS, TimeUnit.MILLISECONDS))
        val vault = requireNotNull(answer.get()).completed()
        try {
            vault.replace(credential).completed()
            assertEquals(credential, vault.read().completed())
        } finally {
            vault.close()
            assertTrue(vault.awaitClosed(CLOSE_TIMEOUT_MILLIS))
        }
        assertEquals(emptyList<String>(), faults.origins)
        Log.i(TAG, "a delivered vault stored and read back its own record")
    }

    @Test
    fun aRecipientThatThrowsWhileTakingAVaultLeavesNoneHoldingTheRecord() {
        val scope = run.scope()
        val offered = CountDownLatch(1)
        val recipient =
            VaultRecipient<CredentialVault> { _ ->
                offered.countDown()
                throw IllegalStateException(QUOTED_INPUT)
            }

        SlipboxVault.openAsync(context, run.namespace, scope, recipient, faults)

        assertTrue(offered.await(ANSWER_TIMEOUT_MILLIS, TimeUnit.MILLISECONDS))
        val credential = StoredCredential(syntheticToken("access"))
        val reopened = granted(scope)
        try {
            reopened.replace(credential).completed()
            assertEquals(credential, reopened.read().completed())
        } finally {
            reopened.close()
            assertTrue(reopened.awaitClosed(CLOSE_TIMEOUT_MILLIS))
        }
        assertEquals(listOf(IllegalStateException::class.java.name), faults.origins)
        for (reported in faults.origins) {
            assertFalse(reported, reported.contains(QUOTED_INPUT))
            assertFalse(reported, reported.contains(TOKEN_MARKER))
        }
        Log.i(TAG, "a handoff that threw released its record and reported only a type")
    }

    /** Failed handoff closes asynchronously; wait for lease release before reopening. */
    private fun granted(scope: VaultScope): CredentialVault {
        val deadline = System.currentTimeMillis() + CLOSE_TIMEOUT_MILLIS
        while (true) {
            when (val outcome = SlipboxVault.open(context, run.namespace, scope)) {
                is VaultOutcome.Completed -> return outcome.value
                is VaultOutcome.Failed -> {
                    assertEquals(VaultFailure.VaultHeld, outcome.failure)
                    assertTrue(
                        "the record of a vault nobody took was never released",
                        System.currentTimeMillis() < deadline,
                    )
                    Thread.sleep(RELEASE_POLL_MILLIS)
                }
            }
        }
    }

    /** The delivery faults of one test, by type as the vault reports them. */
    private class ProbeDeliveryFaults : VaultDeliveryFaults {

        private val reported = CopyOnWriteArrayList<String>()

        val origins: List<String>
            get() = reported.toList()

        override fun onDeliveryFault(origin: String) {
            reported.add(origin)
        }
    }

    private companion object {

        const val TAG = "SlipboxVaultProbe"

        /** Long enough for a thread of its own to open a vault on a loaded device. */
        const val ANSWER_TIMEOUT_MILLIS = 30_000L

        /** Long enough for accepted work to finish, short enough to end a test. */
        const val CLOSE_TIMEOUT_MILLIS = 10_000L

        const val RELEASE_POLL_MILLIS = 25L

        /** A message a report must not repeat, standing in for any platform text. */
        const val QUOTED_INPUT = "an error message that quotes its input"
    }
}
