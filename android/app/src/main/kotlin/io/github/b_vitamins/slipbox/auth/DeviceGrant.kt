/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth

/** Temporary verification data; both codes are redacted from diagnostics. */
class DeviceGrant internal constructor(
    val userCode: String,
    val verificationUri: String,
    val expiresInSeconds: Long,
    val intervalSeconds: Long,
    internal val deviceCode: String,
) {

    override fun toString(): String =
        "DeviceGrant($verificationUri, code redacted, expires in ${expiresInSeconds}s," +
            " interval ${intervalSeconds}s)"

    internal companion object {


        const val DEFAULT_INTERVAL_SECONDS: Long = 5


        const val MAX_INTERVAL_SECONDS: Long = 60

        const val MAX_LIFETIME_SECONDS: Long = 3600
    }
}
