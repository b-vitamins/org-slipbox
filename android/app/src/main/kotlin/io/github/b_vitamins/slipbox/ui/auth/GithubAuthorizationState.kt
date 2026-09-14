/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.auth

import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import io.github.b_vitamins.slipbox.auth.AuthorizationListener
import io.github.b_vitamins.slipbox.auth.AuthorizationOutcome
import io.github.b_vitamins.slipbox.auth.BrowserHandoff
import io.github.b_vitamins.slipbox.auth.DeviceGrant
import io.github.b_vitamins.slipbox.auth.GithubApp
import io.github.b_vitamins.slipbox.auth.GithubAuthorizationOwner
import io.github.b_vitamins.slipbox.auth.SystemBrowserHandoff

internal sealed interface AuthorizationPhase {


    object Idle : AuthorizationPhase


    object Requesting : AuthorizationPhase


    data class Verifying(val grant: DeviceGrant) : AuthorizationPhase


    data class Settled(val outcome: AuthorizationOutcome) : AuthorizationPhase
}

/** Composition-local authorization state; temporary codes are not saved. */
@Stable
internal class GithubAuthorizationState(
    internal val owner: GithubAuthorizationOwner,
    private val browser: BrowserHandoff,
    private val installationUrl: String?,
) : AuthorizationListener {

    var phase: AuthorizationPhase by mutableStateOf(AuthorizationPhase.Idle)
        private set


    var browserRefused: Boolean by mutableStateOf(false)
        private set


    val canInstall: Boolean = installationUrl != null


    fun begin() {
        browserRefused = false
        phase = AuthorizationPhase.Requesting
        owner.authorize(this)
    }


    fun openVerification() {
        val waiting = phase as? AuthorizationPhase.Verifying ?: return
        browserRefused = !browser.open(waiting.grant.verificationUri)
    }


    fun openInstallation() {
        val address = installationUrl ?: return
        browserRefused = !browser.open(address)
    }


    fun cancel() {
        owner.running()?.cancel()
        browserRefused = false
        phase = AuthorizationPhase.Idle
    }


    fun dispose() {
        owner.close()
        phase = AuthorizationPhase.Idle
    }

    override fun onVerificationWaiting(grant: DeviceGrant) {
        phase = AuthorizationPhase.Verifying(grant)
    }

    override fun onSettled(outcome: AuthorizationOutcome) {
        phase =
            when (outcome) {
                is AuthorizationOutcome.Withdrawn -> AuthorizationPhase.Idle
                else -> AuthorizationPhase.Settled(outcome)
            }
    }
}

@Composable
internal fun rememberGithubAuthorization(): GithubAuthorizationState {
    val context = LocalContext.current
    val state =
        remember(context) {
            GithubAuthorizationState(
                owner = GithubAuthorizationOwner.packaged(),
                browser = SystemBrowserHandoff(context),
                installationUrl = GithubApp.packaged()?.installationUrl,
            )
        }
    DisposableEffect(state) { onDispose { state.dispose() } }
    return state
}
