/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth

import io.github.b_vitamins.slipbox.BuildConfig

/** Public project registration; absent configuration disables authorization. */
class GithubApp private constructor(val clientId: String, val installationUrl: String?) {

    override fun toString(): String = "GithubApp($clientId)"

    companion object {


        const val PROVIDER_AUTHORITY: String = "github.com"


        const val MAX_CONFIGURATION_LENGTH: Int = 128


        fun packaged(): GithubApp? =
            of(BuildConfig.GITHUB_CLIENT_ID, BuildConfig.GITHUB_INSTALLATION_URL)


        internal fun of(clientId: String, installationUrl: String): GithubApp? {
            if (!isUsable(clientId)) {
                return null
            }
            val address = installationUrl.takeIf { isUsable(it) && it.startsWith(HTTPS) }
            return GithubApp(clientId, address)
        }

        private const val HTTPS = "https://"

        private fun isUsable(value: String): Boolean =
            value.isNotEmpty() &&
                value.length <= MAX_CONFIGURATION_LENGTH &&
                value.all { it.code in PRINTABLE }

        private val PRINTABLE = 0x21..0x7e
    }
}
