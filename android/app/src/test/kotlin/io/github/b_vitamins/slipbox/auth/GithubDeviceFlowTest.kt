/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class GithubDeviceFlowTest {

    private val clock = SteppedClock()

    private val live = { true }

    @Test
    fun theCodeRequestNamesTheClientAndAsksForNothingElse() {
        val transport = RecordedTransport().answers(GithubEndpoint.DEVICE_CODE, ok(deviceCodeBody()))

        val grant = flowOver(transport).requestGrant().requested()

        val request = transport.requestsFor(GithubEndpoint.DEVICE_CODE).single()
        assertEquals(mapOf("client_id" to CLIENT_ID), request.form)
        assertNull("a code request carries no credential", request.bearer)
        assertEquals("only the code endpoint was reached", 1, transport.requests.size)
        assertEquals(USER_CODE, grant.userCode)
        assertEquals(VERIFICATION_URI, grant.verificationUri)
        assertEquals(LIFETIME_SECONDS, grant.expiresInSeconds)
        assertEquals(INTERVAL_SECONDS, grant.intervalSeconds)
    }

    @Test
    fun anAnswerThatNamesNoPaceIsPolledAtTheDocumentedOne() {
        val transport =
            RecordedTransport()
                .answers(GithubEndpoint.DEVICE_CODE, ok(deviceCodeBody(interval = null)))

        val grant = flowOver(transport).requestGrant().requested()

        assertEquals(DeviceGrant.DEFAULT_INTERVAL_SECONDS, grant.intervalSeconds)
    }

    @Test
    fun aVerificationAddressThatIsNotHttpsIsNotAGrant() {
        val body = deviceCodeBody(verificationUri = "http://github.com/login/device")
        val transport = RecordedTransport().answers(GithubEndpoint.DEVICE_CODE, ok(body))

        val fault = flowOver(transport).requestGrant().refusal()

        assertEquals(malformed(AuthorizationStage.DeviceCode, AnswerDefect.FieldUnusable), fault)
    }

    @Test
    fun aLifetimeOrPaceOutsideItsBoundsIsNotAGrant() {
        val unusable = malformed(AuthorizationStage.DeviceCode, AnswerDefect.FieldUnusable)

        assertEquals(unusable, refusalOf(deviceCodeBody(expiresIn = 0)))
        assertEquals(unusable, refusalOf(deviceCodeBody(expiresIn = 7200)))
        assertEquals(unusable, refusalOf(deviceCodeBody(interval = 0)))
        assertEquals(unusable, refusalOf(deviceCodeBody(interval = 61)))
    }

    @Test
    fun anAnswerMissingWhatAGrantNeedsIsNotAGrant() {
        assertEquals(
            malformed(AuthorizationStage.DeviceCode, AnswerDefect.FieldUnusable),
            refusalOf(deviceCodeBody(userCode = "")),
        )
        assertEquals(
            malformed(AuthorizationStage.DeviceCode, AnswerDefect.FieldUnusable),
            refusalOf("""{"user_code":"WDJB-MJHT","verification_uri":"$VERIFICATION_URI"}"""),
        )
    }

    @Test
    fun aRefusalOfTheRegistrationIsNamedAsItself() {
        val fault = refusalOf(errorBody("device_flow_disabled"), status = 400)

        assertEquals(
            AuthorizationFault.RequestRefused(
                AuthorizationStage.DeviceCode,
                GrantRefusal.DeviceFlowDisabled,
            ),
            fault,
        )
    }

    @Test
    fun anUnrecognizedRefusalIsUnclassifiedRatherThanQuoted() {
        val quoted = "a_refusal_this_release_does_not_know"

        val fault = refusalOf(errorBody(quoted), status = 400)

        assertEquals(
            AuthorizationFault.RequestRefused(
                AuthorizationStage.DeviceCode,
                GrantRefusal.Unclassified,
            ),
            fault,
        )
        assertFalse("GitHub's own wording escaped", fault.toString().contains(quoted))
    }

    @Test
    fun anAnswerThatIsNotJsonIsMalformed() {
        assertEquals(
            malformed(AuthorizationStage.DeviceCode, AnswerDefect.NotJson),
            refusalOf("<html><body>maintenance</body></html>"),
        )
    }

    @Test
    fun anAnswerWithNoBodyIsMalformed() {
        assertEquals(
            malformed(AuthorizationStage.DeviceCode, AnswerDefect.NotText),
            refusalOf(""),
        )
    }

    @Test
    fun aStatusThisExchangeHasNoMeaningForIsUnexpected() {
        assertEquals(
            AuthorizationFault.UnexpectedStatus(AuthorizationStage.DeviceCode, 500),
            refusalOf(deviceCodeBody(), status = 500),
        )
        assertEquals(
            AuthorizationFault.UnexpectedStatus(AuthorizationStage.DeviceCode, 302),
            refusalOf(deviceCodeBody(), status = 302),
        )
    }

    @Test
    fun aTransportFaultNamesOnlyTheTypeThePlatformRaised() {
        val origin = "java.net.UnknownHostException"
        val transport =
            RecordedTransport()
                .answers(GithubEndpoint.DEVICE_CODE, AuthorizationReply.Failed(origin))

        val fault = flowOver(transport).requestGrant().refusal()

        assertEquals(
            AuthorizationFault.TransportFailed(AuthorizationStage.DeviceCode, origin),
            fault,
        )
        assertEquals(
            "AuthorizationFault(device code, transport $origin)",
            fault.toString(),
        )
    }

    @Test
    fun everyPollNamesTheDeviceGrantAndCarriesNoSecret() {
        val transport = grantingTransport()

        flowOver(transport).awaitGrant(testGrant(), live).authorized()

        val poll = transport.requestsFor(GithubEndpoint.ACCESS_TOKEN).single()
        assertEquals(
            mapOf(
                "client_id" to CLIENT_ID,
                "device_code" to DEVICE_CODE,
                "grant_type" to "urn:ietf:params:oauth:grant-type:device_code",
            ),
            poll.form,
        )
        val keys = transport.requests.flatMap { it.form?.keys.orEmpty() }
        assertTrue("a form named a secret: $keys", keys.none { it.contains("secret") })
        assertTrue("a form asked for a scope: $keys", keys.none { it.contains("scope") })
    }

    @Test
    fun pollingKeepsThePaceGitHubNamedUntilItIssuesTheGrant() {
        val transport = grantingTransport(polls = listOf(pending(), pending(), ok(grantBody())))

        val outcome = flowOver(transport).awaitGrant(testGrant(), live)

        outcome.authorized()
        assertEquals(3, transport.exchangesOf(GithubEndpoint.ACCESS_TOKEN))
        assertEquals(listOf(5_000L, 5_000L, 5_000L), clock.waits)
    }

    @Test
    fun slowDownAddsFiveSecondsToThePaceForEveryPollAfterIt() {
        val transport =
            grantingTransport(
                polls = listOf(refused(errorBody("slow_down")), pending(), ok(grantBody())),
            )

        flowOver(transport).awaitGrant(testGrant(), live).authorized()

        assertEquals(listOf(5_000L, 10_000L, 10_000L), clock.waits)
    }

    @Test
    fun theGrantRetainsTheAccountRefreshTokenAndExpiryADownstreamNeeds() {
        val transport = grantingTransport()

        val authorization = flowOver(transport).awaitGrant(testGrant(), live).authorized()

        assertEquals(VerifiedAccount(ACCOUNT_ID.toString(), LOGIN), authorization.account)
        assertEquals(ACCESS_TOKEN, authorization.credential.accessToken)
        assertEquals(REFRESH_TOKEN, authorization.credential.refreshToken)
        assertEquals(
            SteppedClock.EPOCH_SECONDS + TOKEN_LIFETIME_SECONDS,
            authorization.credential.expiresAtEpochSeconds,
        )
        assertEquals(1, authorization.access.installations)
        assertEquals(RepositorySelection.Selected, authorization.access.selection)
        assertTrue(authorization.access.isInstalled)
    }

    @Test
    fun accountVerificationDoesNotExtendTheTokenExpiry() {
        var epoch = SteppedClock.EPOCH_SECONDS
        val datedClock =
            object : AuthorizationClock by clock {
                override fun epochSeconds(): Long = epoch
            }
        val transport = grantingTransport()
        transport.beforeExchange = {
            if (it.url == GithubEndpoint.USER || it.url == GithubEndpoint.INSTALLATIONS) {
                epoch += 20
            }
        }

        val authorization =
            GithubDeviceFlow(testApp(), transport, datedClock)
                .awaitGrant(testGrant(), live).authorized()

        assertEquals(
            SteppedClock.EPOCH_SECONDS + TOKEN_LIFETIME_SECONDS,
            authorization.credential.expiresAtEpochSeconds,
        )
    }

    @Test
    fun aGrantThatStatesNoExpiryKeepsNone() {
        val transport = grantingTransport(polls = listOf(ok(grantBody(expiresIn = null))))

        val authorization = flowOver(transport).awaitGrant(testGrant(), live).authorized()

        assertNull(authorization.credential.expiresAtEpochSeconds)
    }

    @Test
    fun aGrantOfAnythingOtherThanABearerTokenIsUnusable() {
        val unusable = malformed(AuthorizationStage.Grant, AnswerDefect.FieldUnusable)

        assertEquals(unusable, pollRefusalOf(ok(grantBody(tokenType = "mac"))))
        assertEquals(unusable, pollRefusalOf(ok(grantBody(tokenType = null))))
        assertEquals(unusable, pollRefusalOf(ok(grantBody(accessToken = "a token with spaces"))))
        assertEquals(
            malformed(AuthorizationStage.Grant, AnswerDefect.FieldMissing),
            pollRefusalOf(ok("""{"token_type":"bearer"}""")),
        )
    }

    @Test
    fun anAccountThatDeclinedAtGitHubIsDeclined() {
        val transport = grantingTransport(polls = listOf(refused(errorBody("access_denied"))))

        val outcome = flowOver(transport).awaitGrant(testGrant(), live)

        assertEquals(AuthorizationOutcome.Declined, outcome)
        assertEquals("nothing was read with a grant", 0, transport.exchangesOf(GithubEndpoint.USER))
    }

    @Test
    fun aCodeGitHubHasExpiredIsExpired() {
        val transport = grantingTransport(polls = listOf(refused(errorBody("expired_token"))))

        assertEquals(AuthorizationOutcome.Expired, flowOver(transport).awaitGrant(testGrant(), live))
    }

    @Test
    fun aCodeThatOutlivesItsLifetimeExpiresWithoutAFurtherPoll() {
        val transport = grantingTransport(polls = listOf(pending()))

        val outcome = flowOver(transport).awaitGrant(testGrant(expiresInSeconds = 5), live)

        assertEquals(AuthorizationOutcome.Expired, outcome)
        assertEquals(0, transport.exchangesOf(GithubEndpoint.ACCESS_TOKEN))
    }

    @Test
    fun aRefusalOfThePollItselfIsReportedAtTheGrantStage() {
        val transport = grantingTransport(polls = listOf(refused(errorBody("incorrect_device_code"))))

        val fault = flowOver(transport).awaitGrant(testGrant(), live).unavailable()

        assertEquals(
            AuthorizationFault.RequestRefused(
                AuthorizationStage.Grant,
                GrantRefusal.DeviceCodeUnrecognized,
            ),
            fault,
        )
    }

    @Test
    fun anAttemptThatIsNoLongerLiveStopsBeforeItPolls() {
        val transport = grantingTransport()

        val outcome = flowOver(transport).awaitGrant(testGrant()) { false }

        assertEquals(AuthorizationOutcome.Withdrawn, outcome)
        assertEquals(0, transport.requests.size)
        assertEquals(emptyList<Long>(), clock.waits)
    }

    @Test
    fun anInterruptedWaitWithdrawsTheAttempt() {
        val transport = grantingTransport()
        clock.waitsHonoured = 0

        val outcome = flowOver(transport).awaitGrant(testGrant(), live)

        assertEquals(AuthorizationOutcome.Withdrawn, outcome)
        assertEquals(0, transport.requests.size)
    }

    @Test
    fun aWithdrawalWhileTheGrantIsInFlightDeliversNoAccount() {
        val transport = grantingTransport()
        var running = true
        transport.beforeExchange = { if (it.url == GithubEndpoint.ACCESS_TOKEN) running = false }

        val outcome = flowOver(transport).awaitGrant(testGrant()) { running }

        assertEquals(AuthorizationOutcome.Withdrawn, outcome)
        assertEquals(0, transport.exchangesOf(GithubEndpoint.USER))
    }

    @Test
    fun theAccountIsReadWithTheGrantRatherThanNamedByTheReader() {
        val transport = grantingTransport()

        flowOver(transport).awaitGrant(testGrant(), live).authorized()

        val request = transport.requestsFor(GithubEndpoint.USER).single()
        assertEquals(ACCESS_TOKEN, request.bearer)
        assertNull(request.form)
    }

    @Test
    fun anAccountGitHubWillNotNameIsNotDelivered() {
        val transport = grantingTransport(account = ok(accountBody(login = null)))

        val fault = flowOver(transport).awaitGrant(testGrant(), live).unavailable()

        assertEquals(malformed(AuthorizationStage.Account, AnswerDefect.FieldMissing), fault)
        assertEquals(0, transport.exchangesOf(GithubEndpoint.INSTALLATIONS))
    }

    @Test
    fun anAccountWithNoStableIdentityIsNotDelivered() {
        val transport = grantingTransport(account = ok(accountBody(id = 0)))

        val fault = flowOver(transport).awaitGrant(testGrant(), live).unavailable()

        assertEquals(malformed(AuthorizationStage.Account, AnswerDefect.FieldUnusable), fault)
    }

    @Test
    fun anAccountThatCannotBeReachedFailsAtItsOwnStage() {
        val transport = grantingTransport(account = AuthorizationReply.Failed("java.io.IOException"))

        val fault = flowOver(transport).awaitGrant(testGrant(), live).unavailable()

        assertEquals(
            AuthorizationFault.TransportFailed(AuthorizationStage.Account, "java.io.IOException"),
            fault,
        )
    }

    @Test
    fun anAccountWithNoInstallationIsAuthorizedAndSaidToHaveNone() {
        val transport = grantingTransport(installations = ok(installationsBody(0)))

        val authorization = flowOver(transport).awaitGrant(testGrant(), live).authorized()

        assertEquals(0, authorization.access.installations)
        assertEquals(RepositorySelection.None, authorization.access.selection)
        assertFalse(authorization.access.isInstalled)
    }

    @Test
    fun anInstallationOfEveryRepositoryIsSaidToBeThat() {
        val transport =
            grantingTransport(installations = ok(installationsBody(2, listOf("selected", "all"))))

        val authorization = flowOver(transport).awaitGrant(testGrant(), live).authorized()

        assertEquals(2, authorization.access.installations)
        assertEquals(RepositorySelection.All, authorization.access.selection)
    }

    @Test
    fun installationsGitHubDidNotCountAreNotInvented() {
        val transport = grantingTransport(installations = ok(installationsBody(null)))

        val fault = flowOver(transport).awaitGrant(testGrant(), live).unavailable()

        assertEquals(malformed(AuthorizationStage.Installations, AnswerDefect.FieldMissing), fault)
    }

    @Test
    fun anAnswerCarryingFieldsThisReleaseDoesNotKnowIsStillAGrant() {
        val body =
            """{"access_token":"$ACCESS_TOKEN","token_type":"bearer","scope":"","future":{"a":1}}"""
        val transport = grantingTransport(polls = listOf(ok(body)))

        val authorization = flowOver(transport).awaitGrant(testGrant(), live).authorized()

        assertEquals(ACCESS_TOKEN, authorization.credential.accessToken)
    }

    private fun flowOver(transport: AuthorizationTransport): GithubDeviceFlow =
        GithubDeviceFlow(testApp(), transport, clock)


    private fun refusalOf(body: String, status: Int = 200): AuthorizationFault {
        val transport =
            RecordedTransport()
                .answers(GithubEndpoint.DEVICE_CODE, AuthorizationReply.Answered(status, body))
        return GithubDeviceFlow(testApp(), transport, SteppedClock()).requestGrant().refusal()
    }


    private fun pollRefusalOf(reply: AuthorizationReply): AuthorizationFault =
        GithubDeviceFlow(testApp(), grantingTransport(polls = listOf(reply)), SteppedClock())
            .awaitGrant(testGrant()) { true }
            .unavailable()

    private fun malformed(
        stage: AuthorizationStage,
        defect: AnswerDefect,
    ): AuthorizationFault = AuthorizationFault.MalformedAnswer(stage, defect)
}
