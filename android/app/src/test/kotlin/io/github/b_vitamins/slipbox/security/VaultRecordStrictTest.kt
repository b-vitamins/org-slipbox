/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import org.junit.After
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.nio.ByteBuffer

/**
 * What a record is allowed to decrypt into.
 *
 * Each case authenticates real ciphertext under this scope's own key through the
 * production cipher and envelope, so the body reaches the vault the way a tampered
 * or foreign record on a device would, and only the plaintext rules refuse it.
 */
class VaultRecordStrictTest {

    private val vaults = OpenVaults()
    private val keys = FakeVaultKeys()
    private val file = FakeRecordFile()
    private val scope = testScope()
    private val alias = VaultNamespace.Application.aliasFor(scope, GENERATION)
    private val buffers = RecordingBuffers()
    private val vault = vaults.open(scope, keys, file, buffers = buffers).completed()

    @After
    fun closeTheVaults() {
        vaults.closeAll()
    }

    @Test
    fun aPrintableTokenComesBackAsItself() {
        val token = syntheticToken("plain")
        seal(body(token.toByteArray(Charsets.US_ASCII)))

        assertEquals(token, vault.read().completed()?.accessToken)
        assertCleared()
    }

    @Test
    fun anAccessTokenThatIsNotBoundedPrintableTextIsRefused() {
        for ((case, bytes) in MALFORMED) {
            seal(body(bytes))
            val before = requireNotNull(file.record).copyOf()

            assertEquals(
                case,
                VaultFailure.CorruptRecord(RecordDefect.NotPlainTextBody),
                vault.read().refusal(),
            )
            assertArrayEquals(case, before, file.record)
        }
        assertCleared()
    }

    @Test
    fun aRefreshTokenThatIsNotBoundedPrintableTextIsRefused() {
        for ((case, bytes) in MALFORMED) {
            seal(body(PRINTABLE, refresh = bytes))

            assertEquals(
                case,
                VaultFailure.CorruptRecord(RecordDefect.NotPlainTextBody),
                vault.read().refusal(),
            )
        }
        assertCleared()
    }

    @Test
    fun aTokenNoLengthOfThisRecordDeclaresIsRefusedBeforeItIsRead() {
        seal(body(ByteArray(0), refresh = PRINTABLE))

        assertEquals(
            VaultFailure.CorruptRecord(RecordDefect.MalformedBody),
            vault.read().refusal(),
        )
        assertCleared()
    }

    @Test
    fun aBodyDeclaringMoreThanItCarriesIsTruncatedRatherThanBelieved() {
        val whole = body(PRINTABLE)
        val overstated = whole.copyOf()
        ByteBuffer.wrap(overstated).putInt(1, PRINTABLE.size + BEYOND_THE_BODY)
        seal(overstated)

        assertEquals(
            VaultFailure.CorruptRecord(RecordDefect.Truncated),
            vault.read().refusal(),
        )
        assertCleared()
    }

    @Test
    fun aRefusedRecordIsNeitherRewrittenNorDeleted() {
        seal(body(MALFORMED.getValue("an invalid encoding")))
        val before = requireNotNull(file.record).copyOf()

        vault.read().refusal()

        assertArrayEquals(before, file.record)
        assertEquals(0L, file.attempts.toLong())
        assertCleared()
    }

    @Test
    fun theBuffersAPlaintextPassesThroughAreZeroedOnEveryRoute() {
        val credential = StoredCredential(syntheticToken("access"), syntheticToken("refresh"))
        vault.replace(credential).completed()
        assertEquals(credential, vault.read().completed())
        assertTrue(buffers.taken.toString(), buffers.taken >= WRITTEN_AND_READ_BUFFERS)

        vault.replace(StoredCredential("")).refusal()
        seal(body(MALFORMED.getValue("a control byte")))
        vault.read().refusal()

        assertCleared()
    }

    /** Seals [body] under this scope's own key and stores it as its record. */
    private fun seal(body: ByteArray) {
        keys.provision(alias).completed()
        val sealed =
            AesGcmVaultCipher(keys)
                .seal(
                    alias,
                    VaultEnvelope.aadFor(scope, GENERATION),
                    body,
                    KeySource.Existing,
                )
                .completed()
        file.record =
            VaultEnvelope
                .compose(scope, GENERATION, sealed.nonce, sealed.ciphertext)
                .completed()
    }

    private fun body(access: ByteArray, refresh: ByteArray? = null): ByteArray {
        val length =
            FIXED_LENGTH + access.size + (if (refresh == null) 0 else 4 + refresh.size)
        val body = ByteBuffer.allocate(length)
        body.put(VaultRecordBody.VERSION)
        body.putInt(access.size)
        body.put(access)
        if (refresh == null) {
            body.put(0)
        } else {
            body.put(1)
            body.putInt(refresh.size)
            body.put(refresh)
        }
        body.put(0)
        return body.array()
    }

    private fun assertCleared() {
        assertEquals(
            "buffers still carrying plaintext",
            0L,
            buffers.uncleared.toLong(),
        )
    }

    private companion object {

        const val GENERATION = VaultNamespace.FIRST_GENERATION

        /** A version, an access length, a refresh flag and an expiry flag. */
        const val FIXED_LENGTH = 1 + 4 + 1 + 1

        /** Enough beyond the flags after a token to leave the body altogether. */
        const val BEYOND_THE_BODY = 3

        /** Both tokens and the body on the way out, both tokens on the way back. */
        const val WRITTEN_AND_READ_BUFFERS = 5

        val PRINTABLE = "synthetic-printable".toByteArray(Charsets.US_ASCII)

        /**
         * Bodies a device could hold and this vault must not hand back.
         *
         * The names are the cases; none of these is a credential.
         */
        val MALFORMED =
            mapOf(
                "an invalid encoding" to byteArrayOf(0x73, 0x80.toByte(), 0x74),
                "a truncated sequence" to byteArrayOf(0x73, 0xC3.toByte()),
                "an embedded zero byte" to byteArrayOf(0x73, 0x00, 0x74),
                "a control byte" to byteArrayOf(0x73, 0x1F, 0x74),
                "a space" to byteArrayOf(0x73, 0x20, 0x74),
                "a byte above printable ASCII" to
                    byteArrayOf(0x73, 0xE2.toByte(), 0x82.toByte(), 0xAC.toByte()),
            )
    }
}
