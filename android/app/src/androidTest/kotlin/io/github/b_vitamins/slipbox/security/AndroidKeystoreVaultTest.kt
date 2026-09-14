/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import android.os.Build
import android.security.keystore.KeyInfo
import android.security.keystore.KeyProperties
import android.util.Log
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.After
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.security.InvalidAlgorithmParameterException
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit
import javax.crypto.Cipher
import javax.crypto.SecretKeyFactory
import javax.crypto.spec.GCMParameterSpec

/**
 * The vault this device actually builds: keys the platform provider holds, and
 * records the app's own private file system carries.
 *
 * The JVM suite exercises the same cipher and record logic over injected faults;
 * what only a device can answer is checked here, under a name space of this run's
 * own that it deletes again.
 */
@RunWith(AndroidJUnit4::class)
class AndroidKeystoreVaultTest {

    private val instrumentation = InstrumentationRegistry.getInstrumentation()

    private val context = instrumentation.targetContext

    private val run = VaultProbeRun(context)

    @After
    fun releaseThisRun() {
        val remains = run.release()
        assertEquals(remains.aliases.toString(), emptyList<String>(), remains.aliases)
        assertFalse(run.policy.root.path, remains.rootExists)
        Log.i(
            TAG,
            "run released: ${remains.keysRemoved} key(s), ${remains.entriesRemoved} private entry" +
                "(s), ${remains.aliases.size} alias(es) left",
        )
    }

    @Test
    fun aCredentialWrittenOnThisDeviceIsReadBackByAVaultOpenedAgain() {
        val scope = run.scope()
        val credential = StoredCredential(syntheticToken("access"), syntheticToken("refresh"), EXPIRY)

        run.using(scope) { vault ->
            assertNull(vault.read().completed())
            vault.replace(credential).completed()
        }

        val record = run.record(scope)
        assertTrue(record.path, record.isFile)
        val stored = record.readBytes()
        assertEquals(RECORD_VERSION.toLong(), stored[0].toLong())
        assertEquals(FIRST_GENERATION.toLong(), stored[GENERATION_OFFSET].toLong())
        assertEquals(
            scope.digestHex,
            hexOf(stored.copyOfRange(DIGEST_OFFSET, DIGEST_OFFSET + DIGEST_LENGTH)),
        )
        assertEquals(VaultCipher.NONCE_LENGTH.toLong(), stored[NONCE_OFFSET - 1].toLong())
        assertFalse(
            "the record carries a token marker",
            String(stored, Charsets.ISO_8859_1).contains(TOKEN_MARKER),
        )

        run.using(scope) { reopened -> assertEquals(credential, reopened.read().completed()) }
        Log.i(
            TAG,
            "record ${stored.size} bytes, ciphertext ${stored.size - HEADER_LENGTH} bytes," +
                " token marker absent, reopened vault returned the record",
        )
    }

    @Test
    fun theKeyAVaultSealsWithHasNoEncodedFormAndNoAuthenticationOfItsOwn() {
        val scope = run.scope()
        val key = run.keys.provision(run.alias(scope)).completed()

        assertNull("the provider key has an encoded form", key.encoded)
        assertEquals(KeyProperties.KEY_ALGORITHM_AES, key.algorithm)

        val factory = SecretKeyFactory.getInstance(key.algorithm, PROVIDER)
        val info = factory.getKeySpec(key, KeyInfo::class.java) as KeyInfo
        assertEquals(KEY_SIZE.toLong(), info.keySize.toLong())
        assertEquals(listOf(KeyProperties.BLOCK_MODE_GCM), info.blockModes.toList())
        assertEquals(
            listOf(KeyProperties.ENCRYPTION_PADDING_NONE),
            info.encryptionPaddings.toList(),
        )
        assertEquals(KeyProperties.ORIGIN_GENERATED.toLong(), info.origin.toLong())
        assertEquals(
            (KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT).toLong(),
            info.purposes.toLong(),
        )
        assertFalse("the key asks for a device credential", info.isUserAuthenticationRequired)

        // Randomized encryption, which [KeyInfo] does not report, as the provider
        // enforces it: a nonce a caller chose is refused outright.
        val chosen =
            GCMParameterSpec(VaultCipher.TAG_LENGTH_BITS, ByteArray(VaultCipher.NONCE_LENGTH))
        val refused =
            try {
                Cipher.getInstance(TRANSFORMATION).init(Cipher.ENCRYPT_MODE, key, chosen)
                null
            } catch (rejected: InvalidAlgorithmParameterException) {
                rejected
            }
        assertNotNull("the provider let a caller choose the nonce", refused)

        // Where the provider keeps a key is the platform's decision on the device
        // at hand; this run records what it reports and requires nothing of it.
        val level =
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                info.securityLevel.toString()
            } else {
                "unstated below API ${Build.VERSION_CODES.S}"
            }
        Log.i(
            TAG,
            "key ${info.keySize} bits, no encoded form, a caller's nonce refused," +
                " security level $level",
        )
    }

    @Test
    fun aSecondReplacementSealsUnderAFreshNonceAndIsWhatAReadReturns() {
        val scope = run.scope()
        val record = run.record(scope)
        val first = StoredCredential(syntheticToken("access-1"))
        val second = StoredCredential(syntheticToken("access-2"), syntheticToken("refresh-2"), EXPIRY)

        run.using(scope) { vault ->
            vault.replace(first).completed()
            val before = record.readBytes()
            vault.replace(second).completed()
            val after = record.readBytes()

            val nonces = listOf(before, after).map { hexOf(nonceOf(it)) }
            assertNotEquals(nonces[0], nonces[1])
            assertFalse("the record did not change", before.contentEquals(after))
            assertEquals(
                "a replacement moved to another generation",
                before[GENERATION_OFFSET].toLong(),
                after[GENERATION_OFFSET].toLong(),
            )
            assertEquals(second, vault.read().completed())
        }
        assertEquals(listOf(run.alias(scope)), run.aliases())
        Log.i(TAG, "two replacements of one scope carry distinct nonces under one key")
    }

    @Test
    fun aRecordIsRefusedUnderAnotherScopeAndAfterOneAlteredByte() {
        val scope = run.scope()
        run.using(scope) { it.replace(StoredCredential(syntheticToken("access"))).completed() }
        val record = run.record(scope)
        val sealed = record.readBytes()

        val other = run.scope(credentialRef = "probe-credential-2")
        val copied = run.record(other)
        val borrowed = requireNotNull(copied.parentFile)
        assertTrue(copied.path, borrowed.isDirectory || borrowed.mkdirs())
        copied.writeBytes(sealed)
        run.using(other) { borrowing ->
            assertEquals(
                VaultFailure.CorruptRecord(RecordDefect.ScopeMismatch),
                borrowing.read().refusal(),
            )
        }

        val altered = sealed.copyOf()
        altered[altered.size - 1] = (altered[altered.size - 1].toInt() xor 1).toByte()
        record.writeBytes(altered)
        run.using(scope) { tampered ->
            assertEquals(VaultFailure.AuthenticationFailed, tampered.read().refusal())
        }
        Log.i(TAG, "a borrowed and a one-byte-altered ciphertext are both refused")
    }

    @Test
    fun aRecordWhoseKeyIsGoneAsksForReauthorizationAndKeepsItsCiphertext() {
        val scope = run.scope()
        run.using(scope) { it.replace(StoredCredential(syntheticToken("access"))).completed() }
        val record = run.record(scope)
        val sealed = record.readBytes()

        assertTrue(run.keys.delete(run.alias(scope)).completed())

        run.using(scope) { reopened ->
            val failure = reopened.read().refusal()
            assertEquals(VaultFailure.KeyMissing, failure)
            assertTrue(failure.toString(), failure.reauthorize)
        }
        assertArrayEquals(sealed, record.readBytes())
        assertEquals("a read provisioned a key", emptyList<String>(), run.aliases())
        Log.i(TAG, "a missing key left ${sealed.size} ciphertext bytes and provisioned nothing")
    }

    @Test
    fun aRecoveryOnThisDeviceSealsUnderTheNextGenerationAndTakesTheKeyItSupersedes() {
        val scope = run.scope()
        val record = run.record(scope)
        val second = StoredCredential(syntheticToken("access-2"), syntheticToken("refresh-2"), EXPIRY)

        run.using(scope) { it.replace(StoredCredential(syntheticToken("access-1"))).completed() }
        val superseded = record.readBytes()
        assertEquals(listOf(run.alias(scope, FIRST_GENERATION)), run.aliases())

        val recovery = run.using(scope) { it.reauthorize(second).completed() }

        assertEquals(VaultReauthorization(FIRST_GENERATION + 1, 1, null), recovery)
        val stored = record.readBytes()
        assertEquals((FIRST_GENERATION + 1).toLong(), stored[GENERATION_OFFSET].toLong())
        assertEquals(listOf(run.alias(scope, FIRST_GENERATION + 1)), run.aliases())
        run.using(scope) { assertEquals(second, it.read().completed()) }

        // The key of the generation it superseded is gone from the provider, so what
        // that generation sealed no longer opens on this device.
        record.writeBytes(superseded)
        run.using(scope) { assertEquals(VaultFailure.KeyMissing, it.read().refusal()) }
        Log.i(
            TAG,
            "a recovery sealed under generation ${stored[GENERATION_OFFSET]}," +
                " superseded one key and left the earlier ciphertext unopenable",
        )
    }

    @Test
    fun aSecondVaultOverOneRecordIsRefusedUntilTheFirstHasReleasedIt() {
        val scope = run.scope()
        val credential = StoredCredential(syntheticToken("access"))
        val vault = run.vault(scope)
        val refused: VaultFailure
        try {
            vault.replace(credential).completed()
            refused = SlipboxVault.open(context, run.namespace, scope).refusal()
        } finally {
            vault.close()
            assertTrue(vault.awaitClosed(TIMEOUT_MILLIS))
        }

        assertEquals(VaultFailure.VaultHeld, refused)
        run.using(scope) { granted -> assertEquals(credential, granted.read().completed()) }
        Log.i(TAG, "one record on this device admitted one vault, and another once it closed")
    }

    @Test
    fun theMainThreadIsRefusedTheReadItWouldBlockOnAndAnsweredOffIt() {
        val scope = run.scope()
        val credential = StoredCredential(syntheticToken("access"))
        run.using(scope) { vault ->
            vault.replace(credential).completed()

            var main = ""
            val refused = LinkedBlockingQueue<VaultOutcome<StoredCredential?>>()
            instrumentation.runOnMainSync {
                main = Thread.currentThread().name
                refused.add(vault.read())
            }
            assertEquals(
                VaultFailure.ForegroundRefused,
                requireNotNull(refused.poll(TIMEOUT_SECONDS, TimeUnit.SECONDS)).refusal(),
            )

            val answers = LinkedBlockingQueue<Pair<String, VaultOutcome<StoredCredential?>>>()
            instrumentation.runOnMainSync {
                vault.readAsync(
                    VaultRecipient { outcome ->
                        answers.add(Thread.currentThread().name to outcome)
                    },
                )
            }
            val (thread, outcome) = requireNotNull(answers.poll(TIMEOUT_SECONDS, TimeUnit.SECONDS))
            assertEquals(credential, outcome.completed())
            assertNotEquals(main, thread)
            Log.i(TAG, "the main thread was refused and answered on the vault thread instead")
        }
    }

    @Test
    fun theMainThreadIsRefusedTheOpeningItWouldBlockOnAndHandedOneOffIt() {
        val scope = run.scope()
        // Nothing here asserts on the main thread: what it produced is carried off
        // it and examined by the thread this test runs on.
        var main = ""
        val policies = LinkedBlockingQueue<VaultOutcome<PrivateStoragePolicy>>()
        val vaults = LinkedBlockingQueue<VaultOutcome<CredentialVault>>()
        val answers = LinkedBlockingQueue<Pair<String, VaultOutcome<CredentialVault>>>()
        instrumentation.runOnMainSync {
            main = Thread.currentThread().name
            policies.add(SlipboxVault.policy(context, run.namespace))
            vaults.add(SlipboxVault.open(context, scope))
            SlipboxVault.openAsync(
                context,
                run.namespace,
                scope,
                VaultRecipient { outcome -> answers.add(Thread.currentThread().name to outcome) },
            )
        }

        assertEquals(
            VaultFailure.ForegroundRefused,
            requireNotNull(policies.poll(TIMEOUT_SECONDS, TimeUnit.SECONDS)).refusal(),
        )
        assertEquals(
            VaultFailure.ForegroundRefused,
            requireNotNull(vaults.poll(TIMEOUT_SECONDS, TimeUnit.SECONDS)).refusal(),
        )

        val (thread, outcome) = requireNotNull(answers.poll(TIMEOUT_SECONDS, TimeUnit.SECONDS))
        assertNotEquals(main, thread)
        val vault = outcome.completed()
        try {
            assertNull(vault.read().completed())
        } finally {
            vault.close()
            assertTrue(vault.awaitClosed(TIMEOUT_MILLIS))
        }
        Log.i(TAG, "the main thread was refused a private path and a vault, and handed one off it")
    }

    @Test
    fun aRunDeletesItsOwnAliasesAndIsRefusedTheApplicationsAltogether() {
        val first = run.scope()
        val second = run.scope(sourceId = "probe-source-2")
        for (scope in listOf(first, second)) {
            run.using(scope) { it.replace(StoredCredential(syntheticToken("access"))).completed() }
        }
        val others = run.otherAliases()
        assertEquals(run.aliases().toString(), OWNED_ALIASES.toLong(), run.aliases().size.toLong())

        assertEquals(
            VaultFailure.RefusedInput("key name space", InputDefect.Foreign),
            SlipboxVault.keys().releaseNamespace().refusal(),
        )

        assertEquals(OWNED_ALIASES.toLong(), run.keys.releaseNamespace().completed().toLong())
        assertEquals(emptyList<String>(), run.aliases())
        assertEquals(others.toLong(), run.otherAliases().toLong())
        assertEquals(0L, run.keys.releaseNamespace().completed().toLong())
        Log.i(TAG, "released $OWNED_ALIASES alias(es) of this run, $others other alias(es) untouched")
    }

    /** The nonce a stored record carries, at the offset the envelope gives it. */
    private fun nonceOf(record: ByteArray): ByteArray =
        record.copyOfRange(NONCE_OFFSET, NONCE_OFFSET + VaultCipher.NONCE_LENGTH)

    private companion object {

        const val TAG = "SlipboxVaultProbe"

        const val PROVIDER = "AndroidKeyStore"

        const val KEY_SIZE = 256

        const val TRANSFORMATION = "AES/GCM/NoPadding"

        const val TIMEOUT_SECONDS = 5L

        const val TIMEOUT_MILLIS = 10_000L

        const val EXPIRY = 1_700_000L

        /** One alias per scope at one generation, and this run writes two. */
        const val OWNED_ALIASES = 2

        // The durable record layout, read here without the code that writes it: a
        // version, a generation, the scope digest, the nonce length and the nonce.
        const val RECORD_VERSION = 1

        const val FIRST_GENERATION = 1

        const val GENERATION_OFFSET = 1

        const val DIGEST_OFFSET = GENERATION_OFFSET + 1

        const val DIGEST_LENGTH = 32

        const val NONCE_OFFSET = DIGEST_OFFSET + DIGEST_LENGTH + 1

        const val HEADER_LENGTH = NONCE_OFFSET + VaultCipher.NONCE_LENGTH
    }
}
