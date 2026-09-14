/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.ui.auth

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.semantics
import io.github.b_vitamins.slipbox.R
import io.github.b_vitamins.slipbox.auth.AuthorizationOutcome
import io.github.b_vitamins.slipbox.auth.DeviceGrant
import io.github.b_vitamins.slipbox.auth.GithubAuthorization
import io.github.b_vitamins.slipbox.ui.TextControl
import io.github.b_vitamins.slipbox.ui.theme.SlipboxDimensions

@Composable
internal fun AuthorizationPanel(state: GithubAuthorizationState, modifier: Modifier = Modifier) {
    when (val phase = state.phase) {
        is AuthorizationPhase.Idle -> Unit
        is AuthorizationPhase.Requesting ->
            PanelColumn(modifier) {
                Notice(stringResource(R.string.auth_requesting))
            }
        is AuthorizationPhase.Verifying ->
            PanelColumn(modifier) {
                Verification(
                    grant = phase.grant,
                    refused = state.browserRefused,
                    onOpen = state::openVerification,
                    onCancel = state::cancel,
                )
            }
        is AuthorizationPhase.Settled ->
            PanelColumn(modifier) {
                Settled(state = state, outcome = phase.outcome)
            }
    }
}

@Composable
private fun Verification(
    grant: DeviceGrant,
    refused: Boolean,
    onOpen: () -> Unit,
    onCancel: () -> Unit,
) {
    Column(modifier = Modifier.fillMaxWidth().semantics(mergeDescendants = true) {}) {
        Text(
            text = stringResource(R.string.auth_code_name),
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            text = grant.userCode,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onBackground,
        )
    }
    Notice(stringResource(R.string.auth_verification_instruction, grant.verificationUri))
    if (refused) {
        Notice(stringResource(R.string.auth_browser_unavailable), problem = true)
    }
    Row(horizontalArrangement = Arrangement.spacedBy(SlipboxDimensions.headerPaddingHorizontal)) {
        TextControl(label = stringResource(R.string.auth_action_open), onClick = onOpen)
        TextControl(label = stringResource(R.string.auth_action_cancel), onClick = onCancel)
    }
}

@Composable
private fun Settled(
    state: GithubAuthorizationState,
    outcome: AuthorizationOutcome,
) {
    if (outcome is AuthorizationOutcome.Authorized) {
        Authorized(state = state, authorization = outcome.authorization)
        return
    }
    outcome.notice()?.let { Notice(stringResource(it), problem = true) }
    if (outcome.isRetryable()) {
        TextControl(label = stringResource(R.string.auth_action_retry), onClick = state::begin)
    }
}

@Composable
private fun Authorized(
    state: GithubAuthorizationState,
    authorization: GithubAuthorization,
) {
    Notice(stringResource(R.string.auth_authorized, authorization.account.login))
    if (authorization.access.isInstalled) {
        return
    }
    Notice(stringResource(R.string.auth_not_installed))
    if (state.canInstall) {
        if (state.browserRefused) {
            Notice(stringResource(R.string.auth_browser_unavailable), problem = true)
        }
        TextControl(
            label = stringResource(R.string.auth_action_install),
            onClick = state::openInstallation,
        )
    }
}

@Composable
private fun PanelColumn(modifier: Modifier, body: @Composable ColumnScope.() -> Unit) {
    Column(
        modifier = modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(SlipboxDimensions.readingPadding),
        content = body,
    )
}

@Composable
private fun Notice(text: String, problem: Boolean = false) {
    Text(
        text = text,
        style = MaterialTheme.typography.bodyLarge,
        color =
            if (problem) {
                MaterialTheme.colorScheme.error
            } else {
                MaterialTheme.colorScheme.onSurfaceVariant
            },
    )
}
