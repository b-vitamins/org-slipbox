/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.auth

import androidx.annotation.StringRes
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.auth.AuthorizationFault
import io.github.b_vitamins.slipbox.auth.AuthorizationOutcome
import io.github.b_vitamins.slipbox.auth.GrantRefusal

@StringRes
internal fun AuthorizationOutcome.notice(): Int? =
    when (this) {
        is AuthorizationOutcome.Authorized -> null
        is AuthorizationOutcome.Withdrawn -> null
        is AuthorizationOutcome.Declined -> R.string.auth_declined
        is AuthorizationOutcome.Expired -> R.string.auth_expired
        is AuthorizationOutcome.Unavailable -> fault.notice()
    }

@StringRes
internal fun AuthorizationFault.notice(): Int =
    when (this) {
        is AuthorizationFault.ConfigurationUnavailable -> R.string.auth_unconfigured
        is AuthorizationFault.RequestRefused -> R.string.auth_refused
        is AuthorizationFault.TransportFailed -> R.string.auth_unreachable
        is AuthorizationFault.AttemptFailed,
        is AuthorizationFault.UnexpectedStatus,
        is AuthorizationFault.MalformedAnswer,
        -> R.string.auth_unavailable
    }

internal fun AuthorizationOutcome.isRetryable(): Boolean =
    when (this) {
        is AuthorizationOutcome.Authorized -> false
        is AuthorizationOutcome.Withdrawn -> false
        is AuthorizationOutcome.Declined -> true
        is AuthorizationOutcome.Expired -> true
        is AuthorizationOutcome.Unavailable -> fault.isRetryable()
    }

private fun AuthorizationFault.isRetryable(): Boolean =
    when (this) {
        is AuthorizationFault.ConfigurationUnavailable -> false
        is AuthorizationFault.RequestRefused ->
            when (refusal) {
                GrantRefusal.DeviceFlowDisabled -> false
                GrantRefusal.ClientUnrecognized -> false
                GrantRefusal.GrantUnsupported -> false
                GrantRefusal.DeviceCodeUnrecognized -> true
                GrantRefusal.Unclassified -> true
            }
        is AuthorizationFault.TransportFailed -> true
        is AuthorizationFault.AttemptFailed -> true
        is AuthorizationFault.UnexpectedStatus -> true
        is AuthorizationFault.MalformedAnswer -> true
    }
