/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth

import android.content.Context
import io.github.b_vitamins.slipbox.security.CredentialVault
import io.github.b_vitamins.slipbox.security.SlipboxVault
import io.github.b_vitamins.slipbox.security.VaultNamespace
import io.github.b_vitamins.slipbox.security.VaultOutcome
import io.github.b_vitamins.slipbox.security.VaultReauthorization
import io.github.b_vitamins.slipbox.security.VaultRecipient
import io.github.b_vitamins.slipbox.security.VaultScope

/** Asynchronously stores credentials under the verified account and source scope. */
class AuthorizationVault(
    context: Context,
    private val namespace: VaultNamespace = VaultNamespace.Application,
) {

    private val context = context.applicationContext


    fun scopeFor(
        sourceId: String,
        credentialRef: String,
        account: VerifiedAccount,
    ): VaultOutcome<VaultScope> =
        VaultScope.of(
            sourceId = sourceId,
            providerAuthority = GithubApp.PROVIDER_AUTHORITY,
            accountId = account.id,
            credentialRef = credentialRef,
        )


    fun keep(
        sourceId: String,
        credentialRef: String,
        authorization: GithubAuthorization,
        recipient: VaultRecipient<VaultReauthorization>,
    ) {
        val scope =
            when (val composed = scopeFor(sourceId, credentialRef, authorization.account)) {
                is VaultOutcome.Failed -> {
                    recipient.offer(composed)
                    return
                }
                is VaultOutcome.Completed -> composed.value
            }
        val opened =
            VaultRecipient<CredentialVault> { outcome ->
                when (outcome) {
                    is VaultOutcome.Failed -> recipient.offer(outcome)
                    is VaultOutcome.Completed -> write(outcome.value, authorization, recipient)
                }
            }
        SlipboxVault.openAsync(context, namespace, scope, opened)
    }

    private fun write(
        vault: CredentialVault,
        authorization: GithubAuthorization,
        recipient: VaultRecipient<VaultReauthorization>,
    ) {
        val kept =
            VaultRecipient<VaultReauthorization> { answer ->
                try {
                    recipient.offer(answer)
                } finally {
                    vault.close()
                }
            }
        vault.reauthorizeAsync(authorization.credential, kept)
    }
}
