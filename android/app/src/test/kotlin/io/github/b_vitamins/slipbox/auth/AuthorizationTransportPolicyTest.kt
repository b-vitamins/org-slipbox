/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class AuthorizationTransportPolicyTest {

    @Test
    fun onlyTheFourGithubExchangesUseTheirIntendedCredentialChannel() {
        for (url in listOf(GithubEndpoint.DEVICE_CODE, GithubEndpoint.ACCESS_TOKEN)) {
            assertTrue(
                url,
                AuthorizationRequestPolicy.accepts(
                    AuthorizationRequest(url, form = mapOf("synthetic" to "value")),
                ),
            )
            assertFalse(url, AuthorizationRequestPolicy.accepts(AuthorizationRequest(url)))
            assertFalse(
                url,
                AuthorizationRequestPolicy.accepts(AuthorizationRequest(url, bearer = TOKEN)),
            )
        }
        for (url in listOf(GithubEndpoint.USER, GithubEndpoint.INSTALLATIONS)) {
            assertTrue(
                url,
                AuthorizationRequestPolicy.accepts(AuthorizationRequest(url, bearer = TOKEN)),
            )
            assertFalse(url, AuthorizationRequestPolicy.accepts(AuthorizationRequest(url)))
            assertFalse(
                url,
                AuthorizationRequestPolicy.accepts(
                    AuthorizationRequest(url, form = mapOf("synthetic" to "value")),
                ),
            )
        }
    }

    @Test
    fun aCredentialCannotBeRedirectedOrAddressedAnywhereElse() {
        val hostile =
            listOf(
                "http://api.github.com/user",
                "https://api.github.com:444/user",
                "https://github.example/user",
                "https://github.com@example.invalid/user",
                "https://api.github.com/user?access_token=$TOKEN",
                "https://api.github.com/user/../elsewhere",
                "https://example.invalid/redirect/$TOKEN",
            )

        for (url in hostile) {
            val request = AuthorizationRequest(url, bearer = TOKEN)
            assertFalse(url, AuthorizationRequestPolicy.accepts(request))
            assertEquals(
                AuthorizationReply.Failed("request refused"),
                HttpsAuthorizationTransport().exchange(request),
            )
            assertFalse(request.toString(), request.toString().contains(TOKEN))
            assertFalse(request.toString(), request.toString().contains(url))
        }
    }

    private companion object {
        const val TOKEN = "synthetic-authorization-token"
    }
}
