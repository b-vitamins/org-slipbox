/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth.renewal

import io.github.b_vitamins.slipbox.auth.AnswerDefect
import io.github.b_vitamins.slipbox.auth.AuthorizationClock
import io.github.b_vitamins.slipbox.auth.AuthorizationReply
import io.github.b_vitamins.slipbox.auth.AuthorizationRequest
import io.github.b_vitamins.slipbox.auth.AuthorizationTransport
import io.github.b_vitamins.slipbox.auth.GithubApp
import io.github.b_vitamins.slipbox.security.StoredCredential
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

/** Exchanges a device-flow refresh token using the public app registration. */
internal class GithubTokenRefresh(
    private val app: GithubApp,
    private val transport: AuthorizationTransport,
    private val clock: AuthorizationClock,
) {

    fun exchange(refreshToken: String): RefreshExchange {
        val request =
            AuthorizationRequest(
                ACCESS_TOKEN_URL,
                form =
                    mapOf(
                        CLIENT_ID to app.clientId,
                        GRANT_TYPE to REFRESH_GRANT_TYPE,
                        REFRESH_TOKEN to refreshToken,
                    ),
            )
        val body =
            when (val reply = transport.exchange(request)) {
                is AuthorizationReply.Failed ->
                    return unavailable(RenewalFault.TransportFailed(reply.origin))
                is AuthorizationReply.Answered ->
                    when {
                        !isReadable(reply.status) ->
                            return unavailable(RenewalFault.UnexpectedStatus(reply.status))
                        reply.body.isEmpty() -> return malformed(AnswerDefect.NotText)
                        else -> reply.body
                    }
            }
        val reply = decode(body) ?: return malformed(AnswerDefect.NotJson)
        reply.error?.let { return refusalOf(it) }
        val access = reply.accessToken ?: return malformed(AnswerDefect.FieldMissing)
        if (!isUsableToken(access) || !BEARER.equals(reply.tokenType, ignoreCase = true)) {
            return malformed(AnswerDefect.FieldUnusable)
        }
        val rotated = reply.refreshToken
        val lifetime = reply.expiresIn
        if (lifetime != null && rotated == null) {
            return malformed(AnswerDefect.FieldMissing)
        }
        if ((rotated != null && !isUsableToken(rotated)) || (lifetime != null && lifetime <= 0)) {
            return malformed(AnswerDefect.FieldUnusable)
        }
        val expiresAt = lifetime?.let { clock.epochSeconds() + it }
        return RefreshExchange.Rotated(StoredCredential(access, rotated, expiresAt))
    }

    private fun refusalOf(error: String): RefreshExchange =
        when (error) {
            BAD_REFRESH_TOKEN -> RefreshExchange.Refused(ReauthorizationReason.RefreshRejected)
            ACCESS_DENIED -> RefreshExchange.Refused(ReauthorizationReason.AccessRevoked)
            else -> unavailable(RenewalFault.RequestRefused(RenewalRefusal.of(error)))
        }

    private fun unavailable(fault: RenewalFault): RefreshExchange =
        RefreshExchange.Unavailable(fault)

    private fun malformed(defect: AnswerDefect): RefreshExchange =
        unavailable(RenewalFault.MalformedAnswer(defect))

    // Parser messages can quote credential-bearing responses.
    private fun decode(body: String): RefreshReply? =
        try {
            json.decodeFromString(RefreshReply.serializer(), body)
        } catch (fault: IllegalArgumentException) {
            null
        }

    private companion object {

        const val ACCESS_TOKEN_URL = "https://github.com/login/oauth/access_token"

        const val CLIENT_ID = "client_id"

        const val GRANT_TYPE = "grant_type"

        const val REFRESH_TOKEN = "refresh_token"

        const val REFRESH_GRANT_TYPE = "refresh_token"

        const val BEARER = "bearer"

        const val BAD_REFRESH_TOKEN = "bad_refresh_token"

        const val ACCESS_DENIED = "access_denied"

        val json = Json { ignoreUnknownKeys = true }

        fun isReadable(status: Int): Boolean = status == 200 || status in 400..429

        fun isUsableToken(value: String): Boolean =
            value.isNotEmpty() &&
                value.length <= StoredCredential.MAX_TOKEN_LENGTH &&
                value.all { it.code in 0x21..0x7e }
    }
}

internal sealed interface RefreshExchange {

    data class Rotated(val credential: StoredCredential) : RefreshExchange

    data class Refused(val reason: ReauthorizationReason) : RefreshExchange

    data class Unavailable(val fault: RenewalFault) : RefreshExchange
}

@Serializable
private class RefreshReply(
    @SerialName("access_token") val accessToken: String? = null,
    @SerialName("token_type") val tokenType: String? = null,
    @SerialName("expires_in") val expiresIn: Long? = null,
    @SerialName("refresh_token") val refreshToken: String? = null,
    val error: String? = null,
)
