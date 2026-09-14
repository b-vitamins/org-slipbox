/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

/**
 * The authenticated encryption a vault seals its records with.
 *
 * A record is sealed under the key named by an alias and the additional data of
 * its scope, and can be opened only under both. The nonce belongs to the
 * ciphertext it produced and is stored beside it.
 */
interface VaultCipher {

    /**
     * Seals [plaintext] under [alias] and [aad], taking that key as [key] says.
     *
     * Whether a seal may create its key is the caller's to decide, not this
     * interface's: writing over ciphertext whose key is gone must be refused,
     * while authorizing a new record may create one.
     */
    fun seal(
        alias: String,
        aad: ByteArray,
        plaintext: ByteArray,
        key: KeySource,
    ): VaultOutcome<SealedRecord>

    /**
     * Opens [ciphertext] under [alias], [aad] and [nonce]. Creates no key: an
     * absent one is reported so that existing ciphertext is never written over.
     */
    fun open(
        alias: String,
        aad: ByteArray,
        nonce: ByteArray,
        ciphertext: ByteArray,
    ): VaultOutcome<ByteArray>

    companion object {

        /** AES-GCM's nonce length in bytes, as the platform generates it. */
        const val NONCE_LENGTH: Int = 12

        /** AES-GCM's authentication tag length in bits. */
        const val TAG_LENGTH_BITS: Int = 128
    }
}

/** Where a seal takes its key from. */
enum class KeySource {
    /** Only a key that already exists; a missing or invalidated one is refused. */
    Existing,

    /** The existing key, or a new one this write authorizes. */
    Provisioned,
}

/** One sealed record: the ciphertext with its tag, and the nonce that sealed it. */
class SealedRecord(val nonce: ByteArray, val ciphertext: ByteArray)
