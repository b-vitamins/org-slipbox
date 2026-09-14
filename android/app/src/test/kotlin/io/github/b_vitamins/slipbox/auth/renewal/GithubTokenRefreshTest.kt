/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth.renewal

import io.github.b_vitamins.slipbox.auth.AnswerDefect
import io.github.b_vitamins.slipbox.auth.AuthorizationReply
import io.github.b_vitamins.slipbox.auth.CLIENT_ID
import io.github.b_vitamins.slipbox.auth.GithubEndpoint
import io.github.b_vitamins.slipbox.auth.REFRESH_TOKEN
import io.github.b_vitamins.slipbox.auth.RecordedTransport
import io.github.b_vitamins.slipbox.auth.errorBody
import io.github.b_vitamins.slipbox.auth.ok
import io.github.b_vitamins.slipbox.auth.refused
import io.github.b_vitamins.slipbox.auth.testApp
import io.github.b_vitamins.slipbox.security.StoredCredential
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class GithubTokenRefreshTest {

    private val transport = RecordedTransport()
    private val clock = MovingClock()

    @Test
    fun rotatesBothTokensAndDatesTheExpiryAtTheAnswer() {
        val requested = clock.epoch
        transport.answers(GithubEndpoint.ACCESS_TOKEN, ok(rotatedBody()))
        transport.beforeExchange = { clock.epoch += ANSWER_SECONDS }

        val exchanged = exchange()

        assertEquals(
            RefreshExchange.Rotated(rotatedCredential(requested + ANSWER_SECONDS)),
            exchanged,
        )
    }

    @Test
    fun sendsTheRegisteredClientAndTheRefreshGrantAlone() {
        transport.answers(GithubEndpoint.ACCESS_TOKEN, ok(rotatedBody()))

        exchange()

        val request = transport.requestsFor(GithubEndpoint.ACCESS_TOKEN).single()
        assertEquals(setOf("client_id", "grant_type", "refresh_token"), request.form?.keys)
        assertEquals(CLIENT_ID, request.form?.get("client_id"))
        assertEquals("refresh_token", request.form?.get("grant_type"))
        assertEquals(REFRESH_TOKEN, request.form?.get("refresh_token"))
        assertNull(request.bearer)
    }

    @Test
    fun rejectsAnExpiringReplyWithoutARotatedRefreshToken() {
        transport.answers(GithubEndpoint.ACCESS_TOKEN, ok(rotatedBody(refreshToken = null)))

        assertEquals(
            RefreshExchange.Unavailable(RenewalFault.MalformedAnswer(AnswerDefect.FieldMissing)),
            exchange(),
        )
    }

    @Test
    fun acceptsANonExpiringReplyWithoutKeepingTheOldRefreshToken() {
        transport.answers(
            GithubEndpoint.ACCESS_TOKEN,
            ok(rotatedBody(refreshToken = null, expiresIn = null)),
        )

        assertEquals(
            RefreshExchange.Rotated(StoredCredential(ROTATED_ACCESS_TOKEN)),
            exchange(),
        )
    }

    @Test
    fun refusesARejectedRefreshToken() {
        transport.answers(GithubEndpoint.ACCESS_TOKEN, refused(errorBody("bad_refresh_token")))

        assertEquals(RefreshExchange.Refused(ReauthorizationReason.RefreshRejected), exchange())
    }

    @Test
    fun refusesWithdrawnAccess() {
        transport.answers(GithubEndpoint.ACCESS_TOKEN, refused(errorBody("access_denied")))

        assertEquals(RefreshExchange.Refused(ReauthorizationReason.AccessRevoked), exchange())
    }

    @Test
    fun reportsARefusalAuthorizingAgainWouldNotFix() {
        transport.answers(
            GithubEndpoint.ACCESS_TOKEN,
            refused(errorBody("incorrect_client_credentials")),
            refused(errorBody("unsupported_grant_type")),
            refused(errorBody("a_refusal_this_client_does_not_know")),
        )

        assertEquals(unavailable(RenewalRefusal.ClientUnrecognized), exchange())
        assertEquals(unavailable(RenewalRefusal.GrantUnsupported), exchange())
        assertEquals(unavailable(RenewalRefusal.Unclassified), exchange())
    }

    @Test
    fun readsNoBodyFromAStatusItDoesNotExpect() {
        val status = 500
        transport.answers(
            GithubEndpoint.ACCESS_TOKEN,
            AuthorizationReply.Answered(status, errorBody("bad_refresh_token")),
        )

        assertEquals(
            RefreshExchange.Unavailable(RenewalFault.UnexpectedStatus(status)),
            exchange(),
        )
    }

    @Test
    fun reportsAnAnswerItCannotUseWithoutQuotingIt() {
        val unusable =
            listOf(
                "" to AnswerDefect.NotText,
                "not an object" to AnswerDefect.NotJson,
                "{}" to AnswerDefect.FieldMissing,
                rotatedBody(accessToken = "two words") to AnswerDefect.FieldUnusable,
                rotatedBody(refreshToken = "two words") to AnswerDefect.FieldUnusable,
                rotatedBody(refreshToken = "") to AnswerDefect.FieldUnusable,
                rotatedBody(expiresIn = 0) to AnswerDefect.FieldUnusable,
                rotatedBody(tokenType = "mac") to AnswerDefect.FieldUnusable,
                rotatedBody(tokenType = null) to AnswerDefect.FieldUnusable,
            )

        for ((body, defect) in unusable) {
            val answered = RecordedTransport().answers(GithubEndpoint.ACCESS_TOKEN, ok(body))
            assertEquals(
                defect.name,
                RefreshExchange.Unavailable(RenewalFault.MalformedAnswer(defect)),
                GithubTokenRefresh(testApp(), answered, clock).exchange(REFRESH_TOKEN),
            )
        }
    }

    @Test
    fun namesOnlyTheTypeOfATransportFault() {
        transport.answers(GithubEndpoint.ACCESS_TOKEN, AuthorizationReply.Failed(FAULT_ORIGIN))

        val exchanged = exchange()

        assertEquals(
            RefreshExchange.Unavailable(RenewalFault.TransportFailed(FAULT_ORIGIN)),
            exchanged,
        )
        val printed = exchanged.toString()
        assertTrue(printed, printed.contains(FAULT_ORIGIN))
        assertFalse(printed, printed.contains(REFRESH_TOKEN))
    }

    private fun exchange(refreshToken: String = REFRESH_TOKEN): RefreshExchange =
        GithubTokenRefresh(testApp(), transport, clock).exchange(refreshToken)

    private fun unavailable(refusal: RenewalRefusal): RefreshExchange =
        RefreshExchange.Unavailable(RenewalFault.RequestRefused(refusal))

    private companion object {

        const val ANSWER_SECONDS = 30L

        const val FAULT_ORIGIN = "java.net.UnknownHostException"
    }
}
