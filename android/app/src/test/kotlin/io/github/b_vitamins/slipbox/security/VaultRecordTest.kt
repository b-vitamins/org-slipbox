/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Test
import java.nio.ByteBuffer

class VaultRecordTest {

    @Test
    fun aCredentialSurvivesTheCodecWithAndWithoutItsOptionalFields() {
        val whole = StoredCredential(syntheticToken("access"), syntheticToken("refresh"), 1_700_000L)
        assertEquals(whole, decode(encode(whole)).completed())

        val bare = StoredCredential(syntheticToken("access"))
        val decoded = decode(encode(bare)).completed()
        assertEquals(bare, decoded)
        assertNull(decoded.refreshToken)
        assertNull(decoded.expiresAtEpochSeconds)

        val dated = StoredCredential(syntheticToken("access"), null, 0L)
        assertEquals(dated, decode(encode(dated)).completed())
    }

    @Test
    fun aTokenTheVaultWouldNotStoreIsRefusedBeforeAnyEncryption() {
        val buffers = RecordingBuffers()
        val refused =
            listOf(
                VaultFailure.RefusedInput("access token", InputDefect.Empty) to
                    StoredCredential(""),
                VaultFailure.RefusedInput("access token", InputDefect.TooLong) to
                    StoredCredential("t".repeat(StoredCredential.MAX_TOKEN_LENGTH + 1)),
                VaultFailure.RefusedInput("refresh token", InputDefect.NotPlainText) to
                    StoredCredential("token", "refresh token"),
                VaultFailure.RefusedInput("refresh token", InputDefect.Empty) to
                    StoredCredential("token", ""),
            )
        for ((failure, credential) in refused) {
            assertEquals(failure, VaultRecordBody.encode(credential, buffers).refusal())
        }

        assertEquals("a refused token was copied anyway", 0L, buffers.taken.toLong())
    }

    @Test
    fun aBodyThatIsNotOneWholeRecordIsRefusedInsteadOfPartlyBelieved() {
        val body = encode(StoredCredential(syntheticToken("access"), null, 1L))

        assertEquals(defect(RecordDefect.Truncated), decode(ByteArray(0)))
        assertEquals(defect(RecordDefect.Truncated), decode(body.copyOf(body.size - 1)))
        assertEquals(defect(RecordDefect.TrailingBytes), decode(body + 0))
        assertEquals(defect(RecordDefect.UnsupportedVersion), decode(mutated(body) { it[0] = 2 }))
        assertEquals(
            defect(RecordDefect.MalformedBody),
            decode(mutated(body) { ByteBuffer.wrap(it).putInt(1, 0) }),
        )
        assertEquals(
            defect(RecordDefect.Truncated),
            decode(
                mutated(body) {
                    ByteBuffer.wrap(it).putInt(1, StoredCredential.MAX_TOKEN_LENGTH)
                },
            ),
        )
        assertEquals(
            defect(RecordDefect.NotPlainTextBody),
            decode(mutated(body) { it[FIRST_TEXT] = 0 }),
        )

        // The byte after the access token says whether a refresh token follows.
        val flag = FIRST_TEXT + accessLength(body)
        assertEquals(defect(RecordDefect.MalformedBody), decode(mutated(body) { it[flag] = 7 }))
    }

    @Test
    fun everyBufferTheCodecTakesForAFieldIsZeroedWhicheverWayItEnds() {
        val buffers = RecordingBuffers()
        val credential = StoredCredential(syntheticToken("access"), syntheticToken("refresh"), 1L)

        val body = VaultRecordBody.encode(credential, buffers).completed()
        assertEquals("a field buffer outlived the body", 1L, buffers.uncleared.toLong())
        assertEquals(credential, VaultRecordBody.decode(body, buffers).completed())
        assertEquals(
            defect(RecordDefect.NotPlainTextBody),
            VaultRecordBody.decode(mutated(body) { it[FIRST_TEXT] = 0 }, buffers),
        )
        assertEquals("a field buffer outlived its field", 1L, buffers.uncleared.toLong())

        // The body is the one buffer its caller zeroes, once it has been sealed.
        body.fill(0)
        assertEquals(0L, buffers.uncleared.toLong())
    }

    @Test
    fun anEnvelopeCarriesTheVersionTheScopeAndTheNonceInTheClearAndNothingElse() {
        val scope = testScope()
        val nonce = ByteArray(VaultCipher.NONCE_LENGTH) { it.toByte() }
        val ciphertext = ByteArray(MIN_CIPHERTEXT_LENGTH) { (it + 1).toByte() }

        val record = VaultEnvelope.compose(scope, GENERATION, nonce, ciphertext).completed()
        assertEquals((HEADER_LENGTH + ciphertext.size).toLong(), record.size.toLong())
        assertEquals(VaultEnvelope.VERSION, record[0])
        assertEquals(GENERATION.toByte(), record[GENERATION_AT])

        val parsed = VaultEnvelope.parse(record, scope).completed()
        assertEquals(GENERATION.toLong(), parsed.generation.toLong())
        assertArrayEquals(VaultEnvelope.aadFor(scope, GENERATION), parsed.aad)
        assertArrayEquals(nonce, parsed.nonce)
        assertArrayEquals(ciphertext, parsed.ciphertext)
    }

    @Test
    fun theAdditionalDataIsTheVersionTheGenerationAndTheScopeItAuthenticates() {
        val aad = VaultEnvelope.aadFor(testScope(), GENERATION)
        assertEquals(AAD_LENGTH.toLong(), aad.size.toLong())
        assertEquals(VaultEnvelope.VERSION, aad[0])
        assertEquals(GENERATION.toByte(), aad[GENERATION_AT])
        assertArrayEquals(testScope().digestBytes(), aad.copyOfRange(DIGEST_AT, aad.size))

        val otherScope = VaultEnvelope.aadFor(testScope(credentialRef = "other"), GENERATION)
        val otherGeneration = VaultEnvelope.aadFor(testScope(), GENERATION + 1)
        assertFalse(aad.contentEquals(otherScope))
        assertFalse("two generations authenticate alike", aad.contentEquals(otherGeneration))
    }

    @Test
    fun aStoredRecordOfAnotherScopeIsRefusedBeforeItIsDecrypted() {
        val record = composed(testScope())

        assertEquals(
            defect(RecordDefect.ScopeMismatch),
            VaultEnvelope.parse(record, testScope(credentialRef = "credential-2")),
        )
    }

    @Test
    fun aGenerationNoKeyAliasNamesIsRefusedRatherThanWrittenOrRead() {
        val scope = testScope()
        val nonce = ByteArray(VaultCipher.NONCE_LENGTH)
        val ciphertext = ByteArray(MIN_CIPHERTEXT_LENGTH)

        for (generation in listOf(BELOW_FIRST_GENERATION, BEYOND_LAST_GENERATION)) {
            assertEquals(
                defect(RecordDefect.UnknownGeneration),
                VaultEnvelope.compose(scope, generation, nonce, ciphertext),
            )
        }
        assertEquals(
            defect(RecordDefect.UnknownGeneration),
            VaultEnvelope.parse(mutated(composed(scope)) { it[GENERATION_AT] = 0 }, scope),
        )
    }

    @Test
    fun theGenerationARecordDeclaresIsReadableOnItsOwnOrNotAtAll() {
        val record = composed(testScope())

        assertEquals(
            GENERATION.toLong(),
            requireNotNull(VaultEnvelope.generationIn(record)).toLong(),
        )
        assertEquals(
            VaultNamespace.LAST_GENERATION.toLong(),
            requireNotNull(VaultEnvelope.generationIn(mutated(record) { it[GENERATION_AT] = -1 }))
                .toLong(),
        )
        assertNull(VaultEnvelope.generationIn(mutated(record) { it[0] = 2 }))
        assertNull(VaultEnvelope.generationIn(mutated(record) { it[GENERATION_AT] = 0 }))
        assertNull(VaultEnvelope.generationIn(ByteArray(1) { VaultEnvelope.VERSION }))
        assertNull(VaultEnvelope.generationIn(ByteArray(0)))
    }

    @Test
    fun aStoredRecordIsBoundedAndSelfDescribing() {
        val scope = testScope()
        val nonce = ByteArray(VaultCipher.NONCE_LENGTH)

        assertEquals(
            defect(RecordDefect.WrongNonceLength),
            VaultEnvelope.compose(
                scope,
                GENERATION,
                ByteArray(VaultCipher.NONCE_LENGTH - 1),
                ByteArray(MIN_CIPHERTEXT_LENGTH),
            ),
        )
        assertEquals(
            defect(RecordDefect.Oversized),
            VaultEnvelope.compose(scope, GENERATION, nonce, ByteArray(VaultEnvelope.MAX_LENGTH)),
        )
        assertEquals(
            defect(RecordDefect.Oversized),
            VaultEnvelope.parse(ByteArray(VaultEnvelope.MAX_LENGTH + 1), scope),
        )
        assertEquals(
            defect(RecordDefect.Truncated),
            VaultEnvelope.parse(ByteArray(HEADER_LENGTH + MIN_CIPHERTEXT_LENGTH - 1), scope),
        )

        val record = composed(scope)
        assertEquals(
            defect(RecordDefect.UnsupportedVersion),
            VaultEnvelope.parse(mutated(record) { it[0] = 2 }, scope),
        )
        assertEquals(
            defect(RecordDefect.WrongNonceLength),
            VaultEnvelope.parse(mutated(record) { it[AAD_LENGTH] = 11 }, scope),
        )
    }

    /** The smallest record [scope] accepts, at the generation these tests use. */
    private fun composed(scope: VaultScope): ByteArray =
        VaultEnvelope
            .compose(
                scope,
                GENERATION,
                ByteArray(VaultCipher.NONCE_LENGTH),
                ByteArray(MIN_CIPHERTEXT_LENGTH),
            ).completed()

    private fun encode(credential: StoredCredential): ByteArray =
        VaultRecordBody.encode(credential, VaultBuffers.Direct).completed()

    private fun decode(body: ByteArray): VaultOutcome<StoredCredential> =
        VaultRecordBody.decode(body, VaultBuffers.Direct)

    private fun defect(defect: RecordDefect): VaultOutcome<Nothing> =
        VaultOutcome.Failed(VaultFailure.CorruptRecord(defect))

    private fun accessLength(body: ByteArray): Int = ByteBuffer.wrap(body).getInt(1)

    private fun mutated(body: ByteArray, edit: (ByteArray) -> Unit): ByteArray =
        body.copyOf().also(edit)

    private companion object {

        /** The generation these records are sealed under, past the first. */
        const val GENERATION = 2

        const val BELOW_FIRST_GENERATION = VaultNamespace.FIRST_GENERATION - 1

        const val BEYOND_LAST_GENERATION = VaultNamespace.LAST_GENERATION + 1

        /** Where the generation sits, in a record and in its additional data. */
        const val GENERATION_AT = 1

        const val DIGEST_AT = GENERATION_AT + 1

        /** The version, the generation, the scope digest and the nonce. */
        const val HEADER_LENGTH = DIGEST_AT + 32 + 1 + VaultCipher.NONCE_LENGTH

        const val AAD_LENGTH = DIGEST_AT + 32

        /** The first byte of the access token, after the version and its length. */
        const val FIRST_TEXT = 1 + 4

        /** One plaintext byte and one tag. */
        const val MIN_CIPHERTEXT_LENGTH = 1 + VaultCipher.TAG_LENGTH_BITS / 8
    }
}
