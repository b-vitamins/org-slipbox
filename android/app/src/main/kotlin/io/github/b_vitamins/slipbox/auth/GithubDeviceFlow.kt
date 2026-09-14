/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth

import io.github.b_vitamins.slipbox.security.StoredCredential
import kotlinx.serialization.DeserializationStrategy
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

/** Blocking GitHub App device flow; the caller owns cancellation and delivery. */
internal class GithubDeviceFlow(
    private val app: GithubApp,
    private val transport: AuthorizationTransport,
    private val clock: AuthorizationClock,
) {


    fun requestGrant(): DeviceCodeOutcome {
        val request =
            AuthorizationRequest(DEVICE_CODE_URL, form = mapOf(CLIENT_ID to app.clientId))
        val body =
            when (val answer = answered(request, AuthorizationStage.DeviceCode)) {
                is Answer.Refused -> return DeviceCodeOutcome.Unavailable(answer.fault)
                is Answer.Body -> answer.text
            }
        val reply =
            decode(body, DeviceCodeReply.serializer())
                ?: return DeviceCodeOutcome.Unavailable(
                    malformed(AuthorizationStage.DeviceCode, AnswerDefect.NotJson),
                )
        reply.error?.let {
            return DeviceCodeOutcome.Unavailable(
                AuthorizationFault.RequestRefused(
                    AuthorizationStage.DeviceCode,
                    GrantRefusal.of(it),
                ),
            )
        }
        val grant = grantOf(reply)
            ?: return DeviceCodeOutcome.Unavailable(
                malformed(AuthorizationStage.DeviceCode, AnswerDefect.FieldUnusable),
            )
        return DeviceCodeOutcome.Requested(grant)
    }

    /** Poll at GitHub's interval until settlement, expiry or withdrawal. */
    fun awaitGrant(grant: DeviceGrant, live: () -> Boolean): AuthorizationOutcome {
        val started = clock.elapsedMillis()
        val lifetimeMillis = grant.expiresInSeconds * MILLIS_PER_SECOND
        var intervalMillis = grant.intervalSeconds * MILLIS_PER_SECOND
        while (true) {
            if (!live()) {
                return AuthorizationOutcome.Withdrawn
            }
            if (!clock.waitFor(intervalMillis)) {
                return AuthorizationOutcome.Withdrawn
            }
            if (!live()) {
                return AuthorizationOutcome.Withdrawn
            }
            if (clock.elapsedMillis() - started >= lifetimeMillis) {
                return AuthorizationOutcome.Expired
            }
            when (val step = poll(grant.deviceCode)) {
                Poll.Pending -> Unit
                Poll.SlowDown -> intervalMillis += SLOW_DOWN_MILLIS
                Poll.Declined -> return AuthorizationOutcome.Declined
                Poll.Expired -> return AuthorizationOutcome.Expired
                is Poll.Unavailable -> return AuthorizationOutcome.Unavailable(step.fault)
                is Poll.Granted -> return complete(step, live)
            }
        }
    }


    private fun poll(deviceCode: String): Poll {
        val request =
            AuthorizationRequest(
                ACCESS_TOKEN_URL,
                form =
                    mapOf(
                        CLIENT_ID to app.clientId,
                        DEVICE_CODE to deviceCode,
                        GRANT_TYPE to DEVICE_GRANT_TYPE,
                    ),
            )
        val body =
            when (val answer = answered(request, AuthorizationStage.Grant)) {
                is Answer.Refused -> return Poll.Unavailable(answer.fault)
                is Answer.Body -> answer.text
            }
        val reply =
            decode(body, GrantReply.serializer())
                ?: return Poll.Unavailable(malformed(AuthorizationStage.Grant, AnswerDefect.NotJson))
        reply.error?.let { error ->
            return when (error) {
                PENDING -> Poll.Pending
                SLOW_DOWN -> Poll.SlowDown
                ACCESS_DENIED -> Poll.Declined
                EXPIRED_TOKEN -> Poll.Expired
                else ->
                    Poll.Unavailable(
                        AuthorizationFault.RequestRefused(
                            AuthorizationStage.Grant,
                            GrantRefusal.of(error),
                        ),
                    )
            }
        }
        val token = reply.accessToken
        if (token == null) {
            return Poll.Unavailable(malformed(AuthorizationStage.Grant, AnswerDefect.FieldMissing))
        }
        if (!isUsableToken(token) || !BEARER.equals(reply.tokenType, ignoreCase = true)) {
            return Poll.Unavailable(malformed(AuthorizationStage.Grant, AnswerDefect.FieldUnusable))
        }
        val refresh = reply.refreshToken?.takeIf { it.isNotEmpty() }
        if (refresh != null && !isUsableToken(refresh)) {
            return Poll.Unavailable(malformed(AuthorizationStage.Grant, AnswerDefect.FieldUnusable))
        }
        val expiresAt = reply.expiresIn?.let { clock.epochSeconds() + it }
        return Poll.Granted(token, refresh, expiresAt)
    }


    private fun complete(granted: Poll.Granted, live: () -> Boolean): AuthorizationOutcome {
        if (!live()) {
            return AuthorizationOutcome.Withdrawn
        }
        val account =
            when (val read = readAccount(granted.accessToken)) {
                is Read.Refused -> return AuthorizationOutcome.Unavailable(read.fault)
                is Read.Value -> read.value
            }
        if (!live()) {
            return AuthorizationOutcome.Withdrawn
        }
        val access =
            when (val read = readInstallations(granted.accessToken)) {
                is Read.Refused -> return AuthorizationOutcome.Unavailable(read.fault)
                is Read.Value -> read.value
            }
        val credential =
            StoredCredential(
                accessToken = granted.accessToken,
                refreshToken = granted.refreshToken,
                expiresAtEpochSeconds = granted.expiresAtEpochSeconds,
            )
        return AuthorizationOutcome.Authorized(
            GithubAuthorization(account, access, credential),
        )
    }

    private fun readAccount(accessToken: String): Read<VerifiedAccount> {
        val stage = AuthorizationStage.Account
        val body =
            when (val answer = answered(AuthorizationRequest(USER_URL, bearer = accessToken), stage)) {
                is Answer.Refused -> return Read.Refused(answer.fault)
                is Answer.Body -> answer.text
            }
        val reply =
            decode(body, AccountReply.serializer())
                ?: return Read.Refused(malformed(stage, AnswerDefect.NotJson))
        val login = reply.login
        val id = reply.id
        if (login == null || id == null) {
            return Read.Refused(malformed(stage, AnswerDefect.FieldMissing))
        }
        if (login.isEmpty() || login.length > MAX_FIELD_LENGTH || id <= 0) {
            return Read.Refused(malformed(stage, AnswerDefect.FieldUnusable))
        }
        return Read.Value(VerifiedAccount(id.toString(), login))
    }

    private fun readInstallations(accessToken: String): Read<InstallationAccess> {
        val stage = AuthorizationStage.Installations
        val request = AuthorizationRequest(INSTALLATIONS_URL, bearer = accessToken)
        val body =
            when (val answer = answered(request, stage)) {
                is Answer.Refused -> return Read.Refused(answer.fault)
                is Answer.Body -> answer.text
            }
        val reply =
            decode(body, InstallationsReply.serializer())
                ?: return Read.Refused(malformed(stage, AnswerDefect.NotJson))
        val count = reply.totalCount ?: return Read.Refused(malformed(stage, AnswerDefect.FieldMissing))
        if (count < 0) {
            return Read.Refused(malformed(stage, AnswerDefect.FieldUnusable))
        }
        val selection =
            when {
                count == 0 -> RepositorySelection.None
                reply.installations.any { it.repositorySelection == SELECTION_ALL } ->
                    RepositorySelection.All
                else -> RepositorySelection.Selected
            }
        return Read.Value(InstallationAccess(count, selection))
    }


    private fun answered(request: AuthorizationRequest, stage: AuthorizationStage): Answer =
        when (val reply = transport.exchange(request)) {
            is AuthorizationReply.Failed ->
                Answer.Refused(AuthorizationFault.TransportFailed(stage, reply.origin))
            is AuthorizationReply.Answered ->
                when {
                    !isReadable(reply.status) ->
                        Answer.Refused(AuthorizationFault.UnexpectedStatus(stage, reply.status))
                    reply.body.isEmpty() -> Answer.Refused(malformed(stage, AnswerDefect.NotText))
                    else -> Answer.Body(reply.body)
                }
        }

    private fun grantOf(reply: DeviceCodeReply): DeviceGrant? {
        val userCode = reply.userCode ?: return null
        val deviceCode = reply.deviceCode ?: return null
        val verificationUri = reply.verificationUri ?: return null
        val lifetime = reply.expiresIn ?: return null
        val interval = reply.interval ?: DeviceGrant.DEFAULT_INTERVAL_SECONDS
        if (!isUsableCode(userCode) || !isUsableToken(deviceCode)) {
            return null
        }
        if (!verificationUri.startsWith(HTTPS) || verificationUri.length > MAX_FIELD_LENGTH) {
            return null
        }
        if (lifetime !in 1..DeviceGrant.MAX_LIFETIME_SECONDS) {
            return null
        }
        if (interval !in 1..DeviceGrant.MAX_INTERVAL_SECONDS) {
            return null
        }
        return DeviceGrant(userCode, verificationUri, lifetime, interval, deviceCode)
    }

    // Parser messages can quote credential-bearing responses.
    private fun <T> decode(body: String, serializer: DeserializationStrategy<T>): T? =
        try {
            json.decodeFromString(serializer, body)
        } catch (fault: IllegalArgumentException) {
            null
        }

    private companion object {


        const val DEVICE_CODE_URL = "https://github.com/login/device/code"

        const val ACCESS_TOKEN_URL = "https://github.com/login/oauth/access_token"

        const val USER_URL = "https://api.github.com/user"

        const val INSTALLATIONS_URL = "https://api.github.com/user/installations"

        const val CLIENT_ID = "client_id"

        const val DEVICE_CODE = "device_code"

        const val GRANT_TYPE = "grant_type"

        const val DEVICE_GRANT_TYPE = "urn:ietf:params:oauth:grant-type:device_code"

        const val BEARER = "bearer"

        const val PENDING = "authorization_pending"

        const val SLOW_DOWN = "slow_down"

        const val ACCESS_DENIED = "access_denied"

        const val EXPIRED_TOKEN = "expired_token"

        const val SELECTION_ALL = "all"

        const val HTTPS = "https://"


        const val SLOW_DOWN_MILLIS = 5_000L

        const val MILLIS_PER_SECOND = 1_000L


        const val MAX_FIELD_LENGTH = 128


        fun isReadable(status: Int): Boolean = status == 200 || status in 400..429

        val json = Json { ignoreUnknownKeys = true }

        fun malformed(stage: AuthorizationStage, defect: AnswerDefect): AuthorizationFault =
            AuthorizationFault.MalformedAnswer(stage, defect)

        fun isUsableToken(value: String): Boolean =
            value.isNotEmpty() &&
                value.length <= StoredCredential.MAX_TOKEN_LENGTH &&
                value.all { it.code in 0x21..0x7e }

        fun isUsableCode(value: String): Boolean =
            value.isNotEmpty() &&
                value.length <= MAX_FIELD_LENGTH &&
                value.all { it.code in 0x20..0x7e }
    }


    private sealed interface Answer {

        data class Body(val text: String) : Answer

        data class Refused(val fault: AuthorizationFault) : Answer
    }

    private sealed interface Read<out T> {

        data class Value<out T>(val value: T) : Read<T>

        data class Refused(val fault: AuthorizationFault) : Read<Nothing>
    }

    private sealed interface Poll {

        object Pending : Poll

        object SlowDown : Poll

        object Declined : Poll

        object Expired : Poll

        data class Granted(
            val accessToken: String,
            val refreshToken: String?,
            val expiresAtEpochSeconds: Long?,
        ) : Poll

        data class Unavailable(val fault: AuthorizationFault) : Poll
    }
}

internal sealed interface DeviceCodeOutcome {

    data class Requested(val grant: DeviceGrant) : DeviceCodeOutcome

    data class Unavailable(val fault: AuthorizationFault) : DeviceCodeOutcome
}

@Serializable
private class DeviceCodeReply(
    @SerialName("device_code") val deviceCode: String? = null,
    @SerialName("user_code") val userCode: String? = null,
    @SerialName("verification_uri") val verificationUri: String? = null,
    @SerialName("expires_in") val expiresIn: Long? = null,
    val interval: Long? = null,
    val error: String? = null,
)

@Serializable
private class GrantReply(
    @SerialName("access_token") val accessToken: String? = null,
    @SerialName("token_type") val tokenType: String? = null,
    @SerialName("expires_in") val expiresIn: Long? = null,
    @SerialName("refresh_token") val refreshToken: String? = null,
    val error: String? = null,
)

@Serializable
private class AccountReply(val login: String? = null, val id: Long? = null)

@Serializable
private class InstallationsReply(
    @SerialName("total_count") val totalCount: Int? = null,
    val installations: List<InstallationEntry> = emptyList(),
)

@Serializable
private class InstallationEntry(
    @SerialName("repository_selection") val repositorySelection: String? = null,
)
