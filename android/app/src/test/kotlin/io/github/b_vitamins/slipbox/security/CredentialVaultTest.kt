/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import org.junit.After
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The vault over real AES-GCM, with the key store and the record file in memory.
 *
 * Synthetic tokens are compared and searched for, never printed: an assertion
 * message names the case, not the value.
 */
class CredentialVaultTest {

    private val vaults = OpenVaults()
    private val keys = FakeVaultKeys()
    private val file = FakeRecordFile()
    private val foreground = FakeForegroundThread()
    private val scope = testScope()
    private val alias = VaultNamespace.Application.aliasFor(scope, FIRST)
    private val vault = vaults.open(scope, keys, file, foreground).completed()

    @After
    fun closeTheVaults() {
        vaults.closeAll()
    }

    @Test
    fun anEmptyVaultHasNothingToReadAndCreatesNothingByReading() {
        assertNull(vault.read().completed())
        assertTrue(keys.aliases.isEmpty())
        assertEquals(0L, file.attempts.toLong())
    }

    @Test
    fun aStoredCredentialComesBackWholeUnderTheKeyOfItsScope() {
        val credential =
            StoredCredential(syntheticToken("access"), syntheticToken("refresh"), 1_700_000L)
        vault.replace(credential).completed()

        assertEquals(credential, vault.read().completed())
        assertEquals(setOf(alias), keys.aliases)
    }

    @Test
    fun theDurableRecordIsCiphertextItsVersionItsGenerationAndItsScope() {
        val credential = StoredCredential(syntheticToken("access"), syntheticToken("refresh"))
        vault.replace(credential).completed()

        val record = requireNotNull(file.record)
        assertFalse("the access token is in the record", record.carries(credential.accessToken))
        assertFalse(
            "the refresh token is in the record",
            record.carries(requireNotNull(credential.refreshToken)),
        )
        assertFalse("a token prefix is in the record", record.carries("synthetic-"))
        assertEquals(VaultEnvelope.VERSION, record[0])
        assertEquals(FIRST.toLong(), record[GENERATION_AT].toLong())
        assertArrayEquals(scope.digestBytes(), record.copyOfRange(DIGEST_AT, AAD_LENGTH))
    }

    @Test
    fun everyReplacementSealsUnderAFreshNonceEvenForOneUnchangedCredential() {
        val credential = StoredCredential(syntheticToken("access"))
        val nonces = mutableSetOf<String>()
        val records = mutableSetOf<String>()
        repeat(REPLACEMENTS) {
            vault.replace(credential).completed()
            val stored = requireNotNull(file.record)
            nonces.add(VaultEnvelope.parse(stored, scope).completed().nonce.toHex())
            records.add(stored.toHex())
        }

        assertEquals(REPLACEMENTS.toLong(), nonces.size.toLong())
        assertEquals(REPLACEMENTS.toLong(), records.size.toLong())
        assertEquals(credential, vault.read().completed())
        assertEquals("a replacement created a second key", setOf(alias), keys.aliases)
    }

    @Test
    fun aRecordOfAnotherScopeIsRefusedEvenSharingThisKeyStore() {
        vault.replace(StoredCredential(syntheticToken("access"))).completed()
        val elsewhere = FakeRecordFile()
        elsewhere.record = requireNotNull(file.record).copyOf()
        val other = testScope(credentialRef = "credential-2")

        val second = vaults.open(other, keys, elsewhere).completed()

        assertEquals(
            VaultFailure.CorruptRecord(RecordDefect.ScopeMismatch),
            second.read().refusal(),
        )
        assertNotEquals(alias, VaultNamespace.Application.aliasFor(other, FIRST))
    }

    @Test
    fun aRecordSealedForOneScopeDoesNotAuthenticateUnderAnother() {
        val cipher = AesGcmVaultCipher(keys)
        val sealed =
            cipher
                .seal(
                    alias,
                    VaultEnvelope.aadFor(scope, FIRST),
                    byteArrayOf(1, 2, 3),
                    KeySource.Provisioned,
                )
                .completed()

        // One key, one nonce, one ciphertext: only the authenticated scope differs.
        val opened =
            cipher.open(
                alias,
                VaultEnvelope.aadFor(testScope(credentialRef = "credential-2"), FIRST),
                sealed.nonce,
                sealed.ciphertext,
            )
        assertEquals(VaultFailure.AuthenticationFailed, opened.refusal())
    }

    @Test
    fun aRecordSealedUnderOneGenerationDoesNotAuthenticateUnderAnother() {
        val cipher = AesGcmVaultCipher(keys)
        val sealed =
            cipher
                .seal(
                    alias,
                    VaultEnvelope.aadFor(scope, FIRST),
                    byteArrayOf(1, 2, 3),
                    KeySource.Provisioned,
                )
                .completed()

        // One key, one nonce, one ciphertext: only the authenticated generation differs.
        val opened =
            cipher.open(
                alias,
                VaultEnvelope.aadFor(scope, FIRST + 1),
                sealed.nonce,
                sealed.ciphertext,
            )
        assertEquals(VaultFailure.AuthenticationFailed, opened.refusal())
    }

    @Test
    fun aRecordAlteredAnywhereDoesNotAuthenticate() {
        vault.replace(StoredCredential(syntheticToken("access"))).completed()
        val stored = requireNotNull(file.record)

        for (index in listOf(NONCE_AT, HEADER_LENGTH, stored.size - 1)) {
            file.record = flipped(stored, index)
            assertEquals(
                "byte $index was accepted",
                VaultFailure.AuthenticationFailed,
                vault.read().refusal(),
            )
        }
    }

    @Test
    fun aRecordRelabelledToAnotherGenerationIsRefusedRatherThanOpened() {
        vault.replace(StoredCredential(syntheticToken("access"))).completed()
        val stored = requireNotNull(file.record).copyOf()

        file.record = relabelled(stored, 0)
        assertEquals(
            VaultFailure.CorruptRecord(RecordDefect.UnknownGeneration),
            vault.read().refusal(),
        )

        file.record = relabelled(stored, FIRST + 1)
        assertEquals(VaultFailure.KeyMissing, vault.read().refusal())
        assertEquals("a read created a key", setOf(alias), keys.aliases)
    }

    @Test
    fun aRecordThatOutlivesItsKeyIsReportedAndLeftAlone() {
        vault.replace(StoredCredential(syntheticToken("access"))).completed()
        val stored = requireNotNull(file.record).copyOf()

        keys.forget(alias)
        val failure = vault.read().refusal()

        assertEquals(VaultFailure.KeyMissing, failure)
        assertTrue(failure.reauthorize)
        assertArrayEquals(stored, file.record)
        assertTrue("a read created a key", keys.aliases.isEmpty())
    }

    @Test
    fun aReplacementOverARecordWhoseKeyIsGoneIsRefusedRatherThanSealedAgain() {
        val first = StoredCredential(syntheticToken("first"))
        vault.replace(first).completed()
        val stored = requireNotNull(file.record).copyOf()
        val written = file.attempts

        keys.forget(alias)
        val failure = vault.replace(StoredCredential(syntheticToken("second"))).refusal()

        assertEquals(VaultFailure.KeyMissing, failure)
        assertTrue("the previous record was written over", stored.contentEquals(file.record))
        assertTrue("a replacement created a key", keys.aliases.isEmpty())
        assertEquals(
            "the refused replacement reached the file",
            written.toLong(),
            file.attempts.toLong(),
        )
    }

    @Test
    fun aNewKeyUnderTheOldNameCannotReadTheOldRecord() {
        vault.replace(StoredCredential(syntheticToken("access"))).completed()

        keys.forget(alias)
        keys.provision(alias).completed()

        assertEquals(VaultFailure.AuthenticationFailed, vault.read().refusal())
        assertEquals(setOf(alias), keys.aliases)
    }

    @Test
    fun anInvalidatedKeyIsReportedAsTheProviderClassifiesIt() {
        vault.replace(StoredCredential(syntheticToken("access"))).completed()

        keys.spoil(alias)
        keys.classified = VaultFailure.KeyInvalidated
        val failure = vault.read().refusal()

        assertEquals(VaultFailure.KeyInvalidated, failure)
        assertTrue(failure.reauthorize)
        assertNotNull(file.record)
    }

    @Test
    fun aKeyTheProviderCannotExplainIsReportedAsACipherFailure() {
        vault.replace(StoredCredential(syntheticToken("access"))).completed()

        keys.spoil(alias)
        val failure = vault.read().refusal()

        assertTrue(failure.toString(), failure is VaultFailure.CipherUnavailable)
        val unavailable = failure as VaultFailure.CipherUnavailable
        assertEquals(CipherStage.Open, unavailable.stage)
        assertFalse(unavailable.reauthorize)
        // The origin is a type name; a provider message could quote its input.
        assertFalse(unavailable.origin, unavailable.origin.contains(" "))
    }

    @Test
    fun aProviderThatCannotAnswerIsReportedAndNothingIsWritten() {
        keys.failure = VaultFailure.KeyUnavailable(KeyStage.Load, "java.io.IOException")

        assertEquals(
            keys.failure,
            vault.replace(StoredCredential(syntheticToken("access"))).refusal(),
        )
        assertEquals(0L, file.attempts.toLong())
        assertNull(file.record)
    }

    @Test
    fun aCredentialTheVaultWouldNotStoreReachesNeitherKeyNorFile() {
        assertEquals(
            VaultFailure.RefusedInput("access token", InputDefect.Empty),
            vault.replace(StoredCredential("")).refusal(),
        )
        assertEquals(0L, file.attempts.toLong())
        assertTrue(keys.aliases.isEmpty())
    }

    @Test
    fun aReadFailureIsReportedAndLeavesTheVaultUsable() {
        file.readFailure = VaultFailure.ReadFailed("java.io.IOException")
        assertEquals(VaultFailure.ReadFailed("java.io.IOException"), vault.read().refusal())

        file.readFailure = null
        val credential = StoredCredential(syntheticToken("access"))
        vault.replace(credential).completed()
        assertEquals(credential, vault.read().completed())
    }

    @Test
    fun aReadFailureIsAlsoRefusedByAReplacementRatherThanWrittenOver() {
        file.readFailure = VaultFailure.ReadFailed("java.io.IOException")

        assertEquals(
            VaultFailure.ReadFailed("java.io.IOException"),
            vault.replace(StoredCredential(syntheticToken("access"))).refusal(),
        )
        assertEquals(0L, file.attempts.toLong())
        assertTrue(keys.aliases.isEmpty())
    }

    @Test
    fun anOversizedStoredRecordIsRefusedRatherThanRead() {
        file.record = ByteArray(VaultEnvelope.MAX_LENGTH + 1)
        assertEquals(
            VaultFailure.CorruptRecord(RecordDefect.Oversized),
            vault.read().refusal(),
        )
    }

    @Test
    fun aReplacementThatFailsBeforeItCommitsKeepsThePreviousCredential() {
        val first = StoredCredential(syntheticToken("first"))
        vault.replace(first).completed()

        val refusal = VaultFailure.WriteFailed(CommitPhase.BeforeCommit, "java.io.IOException")
        file.writeFailure = refusal
        assertEquals(refusal, vault.replace(StoredCredential(syntheticToken("second"))).refusal())

        file.writeFailure = null
        assertEquals(first, vault.read().completed())
    }

    @Test
    fun aCleanupThatFailsAsWellIsCarriedWithTheFailureItFollowed() {
        val refusal =
            VaultFailure.CleanupAfter(
                VaultFailure.WriteFailed(CommitPhase.BeforeCommit, "java.io.IOException"),
                VaultFailure.CleanupFailed("java.io.IOException"),
            )
        file.writeFailure = refusal

        val reported = vault.replace(StoredCredential(syntheticToken("access"))).refusal()
        assertEquals(refusal, reported)
        assertEquals(VaultFailure.CleanupFailed("java.io.IOException"), reported.cleanup)
        val printed = reported.toString()
        assertTrue(printed, printed.contains("before it committed"))
        assertTrue(printed, printed.contains("while cleaning up"))
    }

    @Test
    fun aRemovalTakesTheKeysAndThenTheRecord() {
        vault.replace(StoredCredential(syntheticToken("access"))).completed()

        assertEquals(VaultRemoval(keysRemoved = 1, recordRemoved = true), vault.remove().completed())
        assertTrue(keys.aliases.isEmpty())
        assertNull(file.record)

        assertEquals(
            VaultRemoval(keysRemoved = 0, recordRemoved = false),
            vault.remove().completed(),
        )
    }

    @Test
    fun aRemovalThatFailsAtTheRecordLeavesCiphertextNothingCanOpen() {
        vault.replace(StoredCredential(syntheticToken("access"))).completed()
        val refusal = VaultFailure.RemovalFailed(RemovalStage.Record, "undeleted record")
        file.deleteFailure = refusal

        assertEquals(refusal, vault.remove().refusal())
        assertTrue("a key outlived the record", keys.aliases.isEmpty())
        assertNotNull(file.record)
        assertEquals(VaultFailure.KeyMissing, vault.read().refusal())
    }

    @Test
    fun aRemovalThatFailsAtTheKeyLeavesTheCredentialReadable() {
        val credential = StoredCredential(syntheticToken("access"))
        vault.replace(credential).completed()
        val refusal = VaultFailure.RemovalFailed(RemovalStage.Key, "undeleted entry")
        keys.sweepFailure = refusal

        assertEquals(refusal, vault.remove().refusal())
        assertNotNull(file.record)

        keys.sweepFailure = null
        assertEquals(credential, vault.read().completed())
    }

    @Test
    fun theForegroundThreadIsRefusedTheWorkItWouldBlockOn() {
        foreground.current = true

        assertEquals(VaultFailure.ForegroundRefused, vault.read().refusal())
        assertEquals(
            VaultFailure.ForegroundRefused,
            vault.replace(StoredCredential(syntheticToken("access"))).refusal(),
        )
        assertEquals(
            VaultFailure.ForegroundRefused,
            vault.reauthorize(StoredCredential(syntheticToken("access"))).refusal(),
        )
        assertEquals(VaultFailure.ForegroundRefused, vault.remove().refusal())
        assertEquals(0L, file.attempts.toLong())
        assertNull(file.record)
        assertTrue(keys.aliases.isEmpty())
    }

    @Test
    fun aClosedVaultAcceptsNoFurtherWork() {
        vault.close()

        assertEquals(VaultFailure.VaultClosed, vault.read().refusal())
        assertEquals(
            VaultFailure.VaultClosed,
            vault.replace(StoredCredential(syntheticToken("access"))).refusal(),
        )
        assertEquals(
            VaultFailure.VaultClosed,
            vault.reauthorize(StoredCredential(syntheticToken("access"))).refusal(),
        )
        assertEquals(VaultFailure.VaultClosed, vault.remove().refusal())
        assertTrue(vault.awaitClosed(CLOSE_TIMEOUT_MILLIS))
        assertNull(file.record)
    }

    /** Whether these bytes carry [text] as it would appear in plaintext. */
    private fun ByteArray.carries(text: String): Boolean =
        String(this, Charsets.ISO_8859_1).contains(text)

    private fun flipped(record: ByteArray, index: Int): ByteArray =
        record.copyOf().also { it[index] = (it[index].toInt() xor 1).toByte() }

    private fun relabelled(record: ByteArray, generation: Int): ByteArray =
        record.copyOf().also { it[GENERATION_AT] = generation.toByte() }

    private companion object {

        const val FIRST = VaultNamespace.FIRST_GENERATION

        /** The generation byte of a record, after the version. */
        const val GENERATION_AT = 1

        /** The first scope digest byte, after the version and the generation. */
        const val DIGEST_AT = 2

        /** The authenticated data of a record: version, generation and digest. */
        const val AAD_LENGTH = DIGEST_AT + 32

        /** The first nonce byte of a record, after its length. */
        const val NONCE_AT = AAD_LENGTH + 1

        /** The first ciphertext byte of a record. */
        const val HEADER_LENGTH = NONCE_AT + VaultCipher.NONCE_LENGTH

        const val REPLACEMENTS = 8
    }
}
