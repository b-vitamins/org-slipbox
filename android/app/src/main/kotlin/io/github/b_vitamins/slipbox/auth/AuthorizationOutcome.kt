/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth

sealed interface AuthorizationOutcome {


    data class Authorized(val authorization: GithubAuthorization) : AuthorizationOutcome


    object Declined : AuthorizationOutcome


    object Expired : AuthorizationOutcome


    object Withdrawn : AuthorizationOutcome


    data class Unavailable(val fault: AuthorizationFault) : AuthorizationOutcome
}

/** Callbacks use the owner's delivery thread; withdrawn attempts drop queued delivery. */
interface AuthorizationListener {


    fun onVerificationWaiting(grant: DeviceGrant)


    fun onSettled(outcome: AuthorizationOutcome)
}
