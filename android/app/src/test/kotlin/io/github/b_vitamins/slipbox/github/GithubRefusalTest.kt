/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.github

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Test

class GithubRefusalTest {

    @Test
    fun anExpiredAuthorizationIsLeftForTheCallerToRenew() {
        val refusal = refusalFor(answered("", status = 401))

        assertEquals(GithubRefusal.AuthorizationExpired(GithubStage.Installations), refusal)
        assertEquals(GithubRecoveryAction.Renew, refusal.recovery?.action)
        assertNull("renewal happens in the caller, not at a page", refusal.recovery?.url)
    }

    @Test
    fun anOrganizationChallengeNamesTheApprovalPageItSupplied() {
        val page = "https://github.com/orgs/synthetic-org/sso?authorization_request=abc"
        val refusal = refusalFor(answered("", status = 403, singleSignOn = "required; url=$page"))

        assertEquals(
            GithubRefusal.SignOnRequired(
                GithubStage.Installations,
                GithubRecovery(GithubRecoveryAction.Approve, page),
            ),
            refusal,
        )
    }

    @Test
    fun aChallengeNamingAnotherSiteFallsBackToTheInstallationPage() {
        val refusal =
            refusalFor(
                answered(
                    "",
                    status = 403,
                    singleSignOn = "required; url=https://github.example/orgs/x/sso",
                ),
            )

        assertEquals(GithubRecoveryAction.Install, refusal.recovery?.action)
        assertEquals(INSTALLATION_URL, refusal.recovery?.url)
    }

    @Test
    fun aRetryRequestOrAnExhaustedQuotaIsARateLimit() {
        val delayed = refusalFor(answered("", status = 403, retryAfter = "60"))
        val exhausted =
            refusalFor(
                answered(
                    "",
                    status = 403,
                    rateLimitRemaining = "0",
                    rateLimitReset = "1700000000",
                ),
            )
        val throttled = refusalFor(answered("", status = 429))

        assertEquals(
            GithubRefusal.RateLimited(GithubStage.Installations, 60L, null),
            delayed,
        )
        assertEquals(
            GithubRefusal.RateLimited(GithubStage.Installations, null, 1700000000L),
            exhausted,
        )
        assertEquals(GithubRefusal.RateLimited(GithubStage.Installations, null, null), throttled)
        assertEquals(GithubRecoveryAction.Wait, throttled.recovery?.action)
        assertNull("waiting is not a page to open", throttled.recovery?.url)
    }

    @Test
    fun aPlainRefusalIsAPermissionGapWithTheInstallationPage() {
        val refusal = refusalFor(answered("", status = 403))

        assertEquals(
            GithubRefusal.AccessRestricted(
                GithubStage.Installations,
                AccessRestriction.PermissionMissing,
                GithubRecovery(GithubRecoveryAction.Install, INSTALLATION_URL),
            ),
            refusal,
        )
    }

    @Test
    fun aHiddenResourceDoesNotClaimToKnowWhyItIsHidden() {
        val refusal = refusalFor(answered("", status = 404))

        assertEquals(
            "GithubRefusal(installations, absent or invisible)",
            refusal.toString(),
        )
        assertEquals(GithubRecoveryAction.Reselect, refusal.recovery?.action)
    }

    @Test
    fun anUnexpectedStatusCarriesOnlyItsCode() {
        val refusal = refusalFor(answered("""{"message":"boom"}""", status = 500))

        assertEquals(GithubRefusal.UnexpectedStatus(GithubStage.Installations, 500), refusal)
        assertEquals("GithubRefusal(installations, status 500)", refusal.toString())
        assertNull("only a later attempt can help", refusal.recovery)
    }

    @Test
    fun aTransportFaultCarriesOnlyItsClassName() {
        val refusal = refusalFor(failed("java.net.SocketTimeoutException"))

        assertEquals(
            "GithubRefusal(installations, transport java.net.SocketTimeoutException)",
            refusal.toString(),
        )
    }

    @Test
    fun anOversizedAnswerIsRefusedRatherThanDecoded() {
        val refusal = refusalFor(answered(installationsBody(installationEntry()), bodyExceeded = true))

        assertEquals(
            GithubRefusal.MalformedAnswer(GithubStage.Installations, GithubDefect.TooLarge),
            refusal,
        )
    }

    @Test
    fun anAnswerThatIsNotJsonOrIsEmptyIsRefused() {
        assertEquals(
            GithubRefusal.MalformedAnswer(GithubStage.Installations, GithubDefect.NotJson),
            refusalFor(answered("<html>gateway</html>")),
        )
        assertEquals(
            GithubRefusal.MalformedAnswer(GithubStage.Installations, GithubDefect.NotText),
            refusalFor(answered("")),
        )
    }

    @Test
    fun aMissingFieldIsRefusedInsteadOfGuessed() {
        assertEquals(
            GithubRefusal.MalformedAnswer(GithubStage.Installations, GithubDefect.FieldUnusable),
            refusalFor(answered("""{"total_count":1}""")),
        )
    }

    @Test
    fun noRefusalPrintsACredentialOrAResponseBody() {
        val body =
            """{"message":"Bad credentials for $ACCESS_TOKEN",""" +
                """"documentation_url":"https://docs.github.com/rest"}"""
        val replies =
            listOf(
                answered(body, status = 401),
                answered(body, status = 403),
                answered(body, status = 404),
                answered(body, status = 429),
                answered(body, status = 500),
                answered(body),
                answered(body, bodyExceeded = true),
                failed("java.io.IOException"),
            )

        for (reply in replies) {
            val printed = refusalFor(reply).toString()
            assertFalse(printed, printed.contains(ACCESS_TOKEN))
            assertFalse(printed, printed.contains("Bad credentials"))
            assertFalse(printed, printed.contains("docs.github.com"))
        }
    }

    private fun refusalFor(reply: GithubApiReply): GithubRefusal =
        testProvider(RecordedApiTransport().answers(installationsUrl(), reply))
            .installations()
            .refusal()
}
