/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

import java.security.MessageDigest

/** Opaque canonical source/provider/account/reference identities for record naming and authentication. */
class VaultScope private constructor(
    val sourceId: String,
    val providerAuthority: String,
    val accountId: String,
    val credentialRef: String,
) {

    private val scopeDigest: ByteArray =
        digestOf(SCOPE_LABEL, sourceId, providerAuthority, accountId, credentialRef)

    /** Credential-scoped key and record identity. */
    val digestHex: String = scopeDigest.toHex()

    /** Account-scoped cache identity, independent of credential replacement. */
    val accountDigestHex: String =
        digestOf(ACCOUNT_LABEL, sourceId, providerAuthority, accountId).toHex()

    /** The authenticated scope bytes, copied so no caller can alter them. */
    internal fun digestBytes(): ByteArray = scopeDigest.copyOf()

    override fun equals(other: Any?): Boolean = other is VaultScope && other.digestHex == digestHex

    override fun hashCode(): Int = digestHex.hashCode()

    override fun toString(): String = "VaultScope($providerAuthority, ${digestHex.take(SHORT)})"

    companion object {

        /** The longest each field may be, matching the canonical identity bound. */
        const val MAX_FIELD_LENGTH: Int = 128

        private const val SCOPE_LABEL = "slipbox.vault.scope.1"

        private const val ACCOUNT_LABEL = "slipbox.vault.account.1"

        private const val SHORT = 16

        fun of(
            sourceId: String,
            providerAuthority: String,
            accountId: String,
            credentialRef: String,
        ): VaultOutcome<VaultScope> {
            val refusal =
                refuseInput("source id", sourceId, MAX_FIELD_LENGTH)
                    ?: refuseInput("provider authority", providerAuthority, MAX_FIELD_LENGTH)
                    ?: refuseInput("account id", accountId, MAX_FIELD_LENGTH)
                    ?: refuseInput("credential reference", credentialRef, MAX_FIELD_LENGTH)
            return if (refusal != null) {
                VaultOutcome.Failed(refusal)
            } else {
                VaultOutcome.Completed(
                    VaultScope(sourceId, providerAuthority, accountId, credentialRef),
                )
            }
        }

        /** Length prefixes distinguish field boundaries before hashing. */
        private fun digestOf(label: String, vararg fields: String): ByteArray {
            val digest = MessageDigest.getInstance("SHA-256")
            digest.update(label.toByteArray(Charsets.UTF_8))
            digest.update(0)
            for (field in fields) {
                val bytes = field.toByteArray(Charsets.UTF_8)
                digest.update(bytes.size.toByte())
                digest.update(bytes)
            }
            return digest.digest()
        }
    }
}

private const val HEX_DIGITS = "0123456789abcdef"

/** These bytes as lowercase hex, without a locale-dependent formatter. */
internal fun ByteArray.toHex(): String {
    val text = StringBuilder(size * 2)
    for (byte in this) {
        val value = byte.toInt() and 0xff
        text.append(HEX_DIGITS[value ushr 4]).append(HEX_DIGITS[value and 0x0f])
    }
    return text.toString()
}
