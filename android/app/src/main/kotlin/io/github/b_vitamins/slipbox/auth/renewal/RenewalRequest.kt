/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth.renewal

import io.github.b_vitamins.slipbox.auth.GithubApp
import io.github.b_vitamins.slipbox.auth.VerifiedAccount
import io.github.b_vitamins.slipbox.security.VaultOutcome
import io.github.b_vitamins.slipbox.security.VaultScope

/** A source-bound credential under a verified GitHub account. */
class RenewalRequest(
    val sourceId: String,
    val account: VerifiedAccount,
    val credentialRef: String,
) {

    internal fun scope(): VaultOutcome<VaultScope> =
        VaultScope.of(
            sourceId = sourceId,
            providerAuthority = GithubApp.PROVIDER_AUTHORITY,
            accountId = account.id,
            credentialRef = credentialRef,
        )
}
