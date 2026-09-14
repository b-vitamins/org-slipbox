/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth

import io.github.b_vitamins.slipbox.security.StoredCredential
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class AuthorizationRedactionTest {

    @Test
    fun aGrantPrintsWhereToGoAndNotWhatToTypeThere() {
        val printed = testGrant().toString()

        assertTrue(printed.contains(VERIFICATION_URI))
        assertTrue(printed.contains("code redacted"))
        assertFalse("the code a reader types escaped", printed.contains(USER_CODE))
        assertFalse("the code the attempt polls with escaped", printed.contains(DEVICE_CODE))
    }

    @Test
    fun aRequestPrintsTheNamesOfItsFormAndNoneOfItsValues() {
        val posted =
            AuthorizationRequest(
                GithubEndpoint.ACCESS_TOKEN,
                form = mapOf("client_id" to CLIENT_ID, "device_code" to DEVICE_CODE),
            )
        val read = AuthorizationRequest(GithubEndpoint.USER, bearer = ACCESS_TOKEN)

        assertEquals(
            "AuthorizationRequest(${GithubEndpoint.ACCESS_TOKEN}," +
                " form [client_id, device_code], unauthorized)",
            posted.toString(),
        )
        assertFalse("a posted code escaped", posted.toString().contains(DEVICE_CODE))
        assertEquals(
            "AuthorizationRequest(${GithubEndpoint.USER}, read, bearer redacted)",
            read.toString(),
        )
        assertFalse("a token escaped", read.toString().contains(ACCESS_TOKEN))
    }

    @Test
    fun anAuthorizationPrintsNoTokenAndNoAccountName() {
        val authorization =
            GithubAuthorization(
                VerifiedAccount(ACCOUNT_ID.toString(), LOGIN),
                InstallationAccess(1, RepositorySelection.Selected),
                StoredCredential(ACCESS_TOKEN, REFRESH_TOKEN, EXPIRY_SECONDS),
            )

        val printed = authorization.toString()

        assertFalse("an access token escaped", printed.contains(ACCESS_TOKEN))
        assertFalse("a refresh token escaped", printed.contains(REFRESH_TOKEN))
        assertFalse("an account name escaped", printed.contains(LOGIN))
        assertTrue("the scoped identity was withheld", printed.contains(ACCOUNT_ID.toString()))
    }

    @Test
    fun everyFaultPrintsItsStageAndADescriptionOfItsOwn() {
        val faults =
            listOf(
                AuthorizationFault.ConfigurationUnavailable to
                    "AuthorizationFault(configuration, no registration)",
                AuthorizationFault.RequestRefused(
                    AuthorizationStage.DeviceCode,
                    GrantRefusal.DeviceFlowDisabled,
                ) to "AuthorizationFault(device code, device flow disabled)",
                AuthorizationFault.TransportFailed(
                    AuthorizationStage.Grant,
                    "java.net.SocketTimeoutException",
                ) to "AuthorizationFault(grant, transport java.net.SocketTimeoutException)",
                AuthorizationFault.UnexpectedStatus(AuthorizationStage.Account, 503) to
                    "AuthorizationFault(account, status 503)",
                AuthorizationFault.AttemptFailed("java.lang.IllegalStateException") to
                    "AuthorizationFault(attempt, attempt java.lang.IllegalStateException)",
                AuthorizationFault.MalformedAnswer(
                    AuthorizationStage.Installations,
                    AnswerDefect.FieldMissing,
                ) to "AuthorizationFault(installations, field missing)",
            )

        for ((fault, printed) in faults) {
            assertEquals(printed, fault.toString())
        }
    }

    @Test
    fun aRefusalGitHubNamedIsMappedRatherThanRepeated() {
        assertEquals(GrantRefusal.DeviceFlowDisabled, GrantRefusal.of("device_flow_disabled"))
        assertEquals(GrantRefusal.ClientUnrecognized, GrantRefusal.of("incorrect_client_credentials"))
        assertEquals(GrantRefusal.DeviceCodeUnrecognized, GrantRefusal.of("incorrect_device_code"))
        assertEquals(GrantRefusal.GrantUnsupported, GrantRefusal.of("unsupported_grant_type"))
        assertEquals(GrantRefusal.Unclassified, GrantRefusal.of("something_else"))
        assertEquals(GrantRefusal.Unclassified, GrantRefusal.of(""))
    }

    private companion object {

        const val EXPIRY_SECONDS = 1_780_028_800L
    }
}
