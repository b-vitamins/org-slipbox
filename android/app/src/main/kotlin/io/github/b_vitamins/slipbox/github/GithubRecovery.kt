/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.github

import io.github.b_vitamins.slipbox.auth.GithubApp
import java.net.URI
import java.net.URISyntaxException

enum class GithubRecoveryAction {

    Renew,

    Install,

    Approve,

    Wait,

    Reselect,

    Refine,
}

class GithubRecovery internal constructor(
    val action: GithubRecoveryAction,
    val url: String? = null,
) {

    override fun equals(other: Any?): Boolean =
        other is GithubRecovery && other.action == action && other.url == url

    override fun hashCode(): Int = 31 * action.hashCode() + (url?.hashCode() ?: 0)

    override fun toString(): String = "GithubRecovery($action, ${url != null})"

    internal companion object {

        fun install(installationUrl: String?): GithubRecovery =
            GithubRecovery(GithubRecoveryAction.Install, providerPage(installationUrl))

        /** Accepts the organization sign-on URL supplied by GitHub's challenge. */
        fun signOn(challenge: String?, installationUrl: String?): GithubRecovery {
            val page = providerPage(urlParameter(challenge))
            return if (page == null) {
                install(installationUrl)
            } else {
                GithubRecovery(GithubRecoveryAction.Approve, page)
            }
        }

        private fun urlParameter(challenge: String?): String? {
            if (challenge == null) {
                return null
            }
            for (parameter in challenge.split(';')) {
                val separator = parameter.indexOf('=')
                if (separator <= 0) {
                    continue
                }
                if (parameter.substring(0, separator).trim() != URL_PARAMETER) {
                    continue
                }
                return parameter.substring(separator + 1).trim().takeIf { it.isNotEmpty() }
            }
            return null
        }

        private fun providerPage(url: String?): String? {
            if (url == null || url.length > MAX_URL_CHARS || !url.startsWith(HTTPS)) {
                return null
            }
            val address =
                try {
                    URI(url)
                } catch (malformed: URISyntaxException) {
                    return null
                }
            if (address.rawUserInfo != null || !HOST.equals(address.host, ignoreCase = true)) {
                return null
            }
            return url
        }

        private const val URL_PARAMETER = "url"

        private const val HTTPS = "https://"

        private const val HOST = GithubApp.PROVIDER_AUTHORITY

        private const val MAX_URL_CHARS = 512
    }
}
