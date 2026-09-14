/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import java.nio.BufferUnderflowException
import java.nio.ByteBuffer
import java.security.MessageDigest

/** Bounded plaintext record codec with symmetric token validation and temporary-buffer clearing. */
internal object VaultRecordBody {

    const val VERSION: Byte = 1

    /** A version, an access length and both presence flags. */
    private const val FIXED_LENGTH = 1 + 4 + 1 + 1

    /** The first byte a stored token may contain. */
    private const val FIRST_PRINTABLE: Byte = 0x21

    /** The last byte a stored token may contain. */
    private const val LAST_PRINTABLE: Byte = 0x7e

    fun encode(credential: StoredCredential, buffers: VaultBuffers): VaultOutcome<ByteArray> {
        val limit = StoredCredential.MAX_TOKEN_LENGTH
        val refusal =
            refuseInput("access token", credential.accessToken, limit)
                ?: credential.refreshToken?.let { refuseInput("refresh token", it, limit) }
        if (refusal != null) {
            return VaultOutcome.Failed(refusal)
        }

        val access = printable(credential.accessToken, buffers)
        val refresh = credential.refreshToken?.let { printable(it, buffers) }
        try {
            val expiry = credential.expiresAtEpochSeconds
            val length =
                FIXED_LENGTH +
                    access.size +
                    (if (refresh == null) 0 else 4 + refresh.size) +
                    (if (expiry == null) 0 else 8)

            val body = ByteBuffer.wrap(buffers.take(length))
            body.put(VERSION)
            body.putInt(access.size)
            body.put(access)
            if (refresh == null) {
                body.put(0)
            } else {
                body.put(1)
                body.putInt(refresh.size)
                body.put(refresh)
            }
            if (expiry == null) {
                body.put(0)
            } else {
                body.put(1)
                body.putLong(expiry)
            }
            return VaultOutcome.Completed(body.array())
        } finally {
            access.fill(0)
            refresh?.fill(0)
        }
    }

    fun decode(body: ByteArray, buffers: VaultBuffers): VaultOutcome<StoredCredential> {
        if (body.size < FIXED_LENGTH + 1) {
            return corrupt(RecordDefect.Truncated)
        }
        val buffer = ByteBuffer.wrap(body)
        try {
            if (buffer.get() != VERSION) {
                return corrupt(RecordDefect.UnsupportedVersion)
            }
            val access = readText(buffer, buffers)

            val refresh =
                when (buffer.get()) {
                    0.toByte() -> null
                    1.toByte() -> readText(buffer, buffers)
                    else -> return corrupt(RecordDefect.MalformedBody)
                }
            val expiry =
                when (buffer.get()) {
                    0.toByte() -> null
                    1.toByte() -> buffer.getLong()
                    else -> return corrupt(RecordDefect.MalformedBody)
                }
            if (buffer.hasRemaining()) {
                return corrupt(RecordDefect.TrailingBytes)
            }
            return VaultOutcome.Completed(StoredCredential(access, refresh, expiry))
        } catch (short: BufferUnderflowException) {
            return corrupt(RecordDefect.Truncated)
        } catch (malformed: MalformedField) {
            return corrupt(malformed.defect)
        }
    }

    /** [text] as one byte per character, which its refusal already guaranteed. */
    private fun printable(text: String, buffers: VaultBuffers): ByteArray {
        val bytes = buffers.take(text.length)
        for (index in text.indices) {
            bytes[index] = text[index].code.toByte()
        }
        return bytes
    }

    /** Validates printable ASCII directly, without lossy character-set decoding. */
    private fun readText(buffer: ByteBuffer, buffers: VaultBuffers): String {
        val length = buffer.getInt()
        if (length < 1 || length > StoredCredential.MAX_TOKEN_LENGTH) {
            throw MalformedField(RecordDefect.MalformedBody)
        }
        val bytes = buffers.take(length)
        try {
            buffer.get(bytes)
            val text = CharArray(length)
            for (index in 0 until length) {
                val byte = bytes[index]
                if (byte < FIRST_PRINTABLE || byte > LAST_PRINTABLE) {
                    throw MalformedField(RecordDefect.NotPlainTextBody)
                }
                text[index] = byte.toInt().toChar()
            }
            return String(text)
        } finally {
            bytes.fill(0)
        }
    }

    private fun corrupt(defect: RecordDefect): VaultOutcome.Failed =
        VaultOutcome.Failed(VaultFailure.CorruptRecord(defect))

    private class MalformedField(val defect: RecordDefect) : Exception()
}

/** Ciphertext envelope; version, generation and scope are cleartext authenticated metadata. */
internal object VaultEnvelope {

    const val VERSION: Byte = 1

    private const val DIGEST_LENGTH = 32

    /** Where the generation byte sits, being the only field read on its own. */
    private const val GENERATION_OFFSET = 1

    /** The version, the generation and the scope digest. */
    private const val AAD_LENGTH = 1 + 1 + DIGEST_LENGTH

    private const val HEADER_LENGTH = AAD_LENGTH + 1 + VaultCipher.NONCE_LENGTH

    /** One plaintext byte and one tag: nothing shorter can be a record. */
    private const val MIN_CIPHERTEXT_LENGTH = 1 + VaultCipher.TAG_LENGTH_BITS / 8

    /** The largest record this vault reads, well above the largest it writes. */
    const val MAX_LENGTH = 16384

    /** The additional data every record of [scope] at [generation] carries. */
    fun aadFor(scope: VaultScope, generation: Int): ByteArray {
        val aad = ByteBuffer.allocate(AAD_LENGTH)
        aad.put(VERSION)
        aad.put(generation.toByte())
        aad.put(scope.digestBytes())
        return aad.array()
    }

    fun compose(
        scope: VaultScope,
        generation: Int,
        nonce: ByteArray,
        ciphertext: ByteArray,
    ): VaultOutcome<ByteArray> {
        if (generation !in VaultNamespace.FIRST_GENERATION..VaultNamespace.LAST_GENERATION) {
            return corrupt(RecordDefect.UnknownGeneration)
        }
        if (nonce.size != VaultCipher.NONCE_LENGTH) {
            return corrupt(RecordDefect.WrongNonceLength)
        }
        if (HEADER_LENGTH + ciphertext.size > MAX_LENGTH) {
            return corrupt(RecordDefect.Oversized)
        }
        val record = ByteBuffer.allocate(HEADER_LENGTH + ciphertext.size)
        record.put(VERSION)
        record.put(generation.toByte())
        record.put(scope.digestBytes())
        record.put(VaultCipher.NONCE_LENGTH.toByte())
        record.put(nonce)
        record.put(ciphertext)
        return VaultOutcome.Completed(record.array())
    }

    fun parse(record: ByteArray, scope: VaultScope): VaultOutcome<Parsed> {
        if (record.size > MAX_LENGTH) {
            return corrupt(RecordDefect.Oversized)
        }
        if (record.size < HEADER_LENGTH + MIN_CIPHERTEXT_LENGTH) {
            return corrupt(RecordDefect.Truncated)
        }
        if (record[0] != VERSION) {
            return corrupt(RecordDefect.UnsupportedVersion)
        }
        val generation = generationIn(record) ?: return corrupt(RecordDefect.UnknownGeneration)
        val stored = record.copyOfRange(GENERATION_OFFSET + 1, AAD_LENGTH)
        if (!MessageDigest.isEqual(stored, scope.digestBytes())) {
            return corrupt(RecordDefect.ScopeMismatch)
        }
        if (record[AAD_LENGTH] != VaultCipher.NONCE_LENGTH.toByte()) {
            return corrupt(RecordDefect.WrongNonceLength)
        }
        return VaultOutcome.Completed(
            Parsed(
                generation = generation,
                aad = record.copyOfRange(0, AAD_LENGTH),
                nonce = record.copyOfRange(AAD_LENGTH + 1, HEADER_LENGTH),
                ciphertext = record.copyOfRange(HEADER_LENGTH, record.size),
            ),
        )
    }

    /** Unauthenticated generation hint; never authorizes overwriting an occupied key alias. */
    fun generationIn(record: ByteArray): Int? {
        if (record.size <= GENERATION_OFFSET || record[0] != VERSION) {
            return null
        }
        val generation = record[GENERATION_OFFSET].toInt() and 0xff
        return if (generation in VaultNamespace.FIRST_GENERATION..VaultNamespace.LAST_GENERATION) {
            generation
        } else {
            null
        }
    }

    private fun corrupt(defect: RecordDefect): VaultOutcome.Failed =
        VaultOutcome.Failed(VaultFailure.CorruptRecord(defect))

    /** The parts of a stored record a cipher needs to open it. */
    class Parsed(
        val generation: Int,
        val aad: ByteArray,
        val nonce: ByteArray,
        val ciphertext: ByteArray,
    )
}
