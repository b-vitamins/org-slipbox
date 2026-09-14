/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth.renewal

import io.github.b_vitamins.slipbox.auth.AuthorizationClock
import io.github.b_vitamins.slipbox.auth.AuthorizationReply
import io.github.b_vitamins.slipbox.auth.AuthorizationRequest
import io.github.b_vitamins.slipbox.auth.AuthorizationTransport
import io.github.b_vitamins.slipbox.auth.DeviceEndpoint
import io.github.b_vitamins.slipbox.auth.EPOCH_SECONDS
import io.github.b_vitamins.slipbox.auth.GithubAuthorization
import io.github.b_vitamins.slipbox.auth.InstallationAccess
import io.github.b_vitamins.slipbox.auth.PROBE_ACCOUNT_ID
import io.github.b_vitamins.slipbox.auth.PROBE_LOGIN
import io.github.b_vitamins.slipbox.auth.RepositorySelection
import io.github.b_vitamins.slipbox.auth.TOKEN_LIFETIME_SECONDS
import io.github.b_vitamins.slipbox.auth.VerifiedAccount
import io.github.b_vitamins.slipbox.security.StoredCredential
import io.github.b_vitamins.slipbox.security.syntheticToken
import java.util.concurrent.CopyOnWriteArrayList

internal class RotatingTransport(
    private val accessToken: String,
    private val refreshToken: String,
) : AuthorizationTransport {

    private val exchanged = CopyOnWriteArrayList<AuthorizationRequest>()

    val requests: List<AuthorizationRequest>
        get() = exchanged.toList()

    override fun exchange(request: AuthorizationRequest): AuthorizationReply {
        exchanged.add(request)
        if (request.url != DeviceEndpoint.ACCESS_TOKEN) {
            throw AssertionError("nothing prepared an exchange of ${request.url}")
        }
        return AuthorizationReply.Answered(OK, rotation())
    }

    private fun rotation(): String =
        "{\"access_token\":\"$accessToken\",\"token_type\":\"bearer\"," +
            "\"expires_in\":$TOKEN_LIFETIME_SECONDS,\"refresh_token\":\"$refreshToken\"}"

    private companion object {

        const val OK = 200
    }
}

internal object SettledClock : AuthorizationClock {

    override fun elapsedMillis(): Long = 0

    override fun epochSeconds(): Long = EPOCH_SECONDS

    override fun waitFor(millis: Long): Boolean = true
}

internal fun spentAuthorization(): GithubAuthorization =
    GithubAuthorization(
        VerifiedAccount(PROBE_ACCOUNT_ID, PROBE_LOGIN),
        InstallationAccess(1, RepositorySelection.Selected),
        StoredCredential(
            accessToken = syntheticToken("access"),
            refreshToken = syntheticToken("refresh"),
            expiresAtEpochSeconds = EPOCH_SECONDS,
        ),
    )
