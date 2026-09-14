/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.security

/** One token/expiry record with redacted diagnostics. String values cannot be erased from the heap. */
class StoredCredential(
    val accessToken: String,
    val refreshToken: String? = null,
    val expiresAtEpochSeconds: Long? = null,
) {

    override fun equals(other: Any?): Boolean =
        other is StoredCredential &&
            other.accessToken == accessToken &&
            other.refreshToken == refreshToken &&
            other.expiresAtEpochSeconds == expiresAtEpochSeconds

    override fun hashCode(): Int {
        var hash = accessToken.hashCode()
        hash = 31 * hash + refreshToken.hashCode()
        hash = 31 * hash + expiresAtEpochSeconds.hashCode()
        return hash
    }

    override fun toString(): String =
        "StoredCredential(access redacted, refresh ${if (refreshToken == null) "absent" else "redacted"}," +
            " expires ${expiresAtEpochSeconds ?: "unstated"})"

    companion object {

        const val MAX_TOKEN_LENGTH: Int = 4096
    }
}
