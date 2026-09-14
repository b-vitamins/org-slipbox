/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.auth

import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.auth.AnswerDefect
import io.github.b_vitamins.slipbox.auth.AuthorizationFault
import io.github.b_vitamins.slipbox.auth.AuthorizationOutcome
import io.github.b_vitamins.slipbox.auth.AuthorizationStage
import io.github.b_vitamins.slipbox.auth.GrantRefusal
import io.github.b_vitamins.slipbox.auth.testAuthorization
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class AuthorizationNoticesTest {

    @Test
    fun anOutcomeWithItsOwnPresentationSaysNothingHere() {
        assertNull(AuthorizationOutcome.Authorized(testAuthorization()).notice())
        assertNull(AuthorizationOutcome.Withdrawn.notice())
    }

    @Test
    fun aRefusalAtGitHubAndAnExpiredCodeAreDistinguished() {
        assertEquals(R.string.auth_declined, AuthorizationOutcome.Declined.notice())
        assertEquals(R.string.auth_expired, AuthorizationOutcome.Expired.notice())
    }

    @Test
    fun anUnavailableOutcomeSaysWhatItsFaultSays() {
        val fault = AuthorizationFault.TransportFailed(AuthorizationStage.Grant, ORIGIN)

        assertEquals(fault.notice(), AuthorizationOutcome.Unavailable(fault).notice())
    }

    @Test
    fun eachFaultSaysWhichKindOfUnavailableItIs() {
        assertEquals(
            R.string.auth_unconfigured,
            AuthorizationFault.ConfigurationUnavailable.notice(),
        )
        assertEquals(
            R.string.auth_refused,
            AuthorizationFault.RequestRefused(
                AuthorizationStage.DeviceCode,
                GrantRefusal.DeviceFlowDisabled,
            ).notice(),
        )
        assertEquals(
            R.string.auth_unreachable,
            AuthorizationFault.TransportFailed(AuthorizationStage.Grant, ORIGIN).notice(),
        )
        assertEquals(
            R.string.auth_unavailable,
            AuthorizationFault.UnexpectedStatus(AuthorizationStage.Account, 503).notice(),
        )
        assertEquals(
            R.string.auth_unavailable,
            AuthorizationFault.AttemptFailed(ORIGIN).notice(),
        )
        assertEquals(
            R.string.auth_unavailable,
            AuthorizationFault.MalformedAnswer(
                AuthorizationStage.Installations,
                AnswerDefect.FieldMissing,
            ).notice(),
        )
    }

    @Test
    fun tryingAgainIsOfferedOnlyWhereItCouldReachAnotherOutcome() {
        assertFalse(AuthorizationOutcome.Authorized(testAuthorization()).isRetryable())
        assertFalse(AuthorizationOutcome.Withdrawn.isRetryable())
        assertTrue(AuthorizationOutcome.Declined.isRetryable())
        assertTrue(AuthorizationOutcome.Expired.isRetryable())
    }

    @Test
    fun aBuildOrARegistrationThatWillNotChangeOffersNoRetry() {
        assertFalse(unavailable(AuthorizationFault.ConfigurationUnavailable).isRetryable())
        assertFalse(unavailable(refused(GrantRefusal.DeviceFlowDisabled)).isRetryable())
        assertFalse(unavailable(refused(GrantRefusal.ClientUnrecognized)).isRetryable())
        assertFalse(unavailable(refused(GrantRefusal.GrantUnsupported)).isRetryable())
        assertTrue(unavailable(refused(GrantRefusal.DeviceCodeUnrecognized)).isRetryable())
        assertTrue(unavailable(refused(GrantRefusal.Unclassified)).isRetryable())
    }

    @Test
    fun somethingThatWentWrongOnceIsWorthTryingAgain() {
        assertTrue(
            unavailable(
                AuthorizationFault.TransportFailed(AuthorizationStage.DeviceCode, ORIGIN),
            ).isRetryable(),
        )
        assertTrue(unavailable(AuthorizationFault.AttemptFailed(ORIGIN)).isRetryable())
        assertTrue(
            unavailable(
                AuthorizationFault.UnexpectedStatus(AuthorizationStage.Grant, 503),
            ).isRetryable(),
        )
        assertTrue(
            unavailable(
                AuthorizationFault.MalformedAnswer(
                    AuthorizationStage.Grant,
                    AnswerDefect.NotJson,
                ),
            ).isRetryable(),
        )
    }

    private fun unavailable(fault: AuthorizationFault): AuthorizationOutcome =
        AuthorizationOutcome.Unavailable(fault)

    private fun refused(refusal: GrantRefusal): AuthorizationFault =
        AuthorizationFault.RequestRefused(AuthorizationStage.Grant, refusal)

    private companion object {

        const val ORIGIN = "java.net.SocketTimeoutException"
    }
}
