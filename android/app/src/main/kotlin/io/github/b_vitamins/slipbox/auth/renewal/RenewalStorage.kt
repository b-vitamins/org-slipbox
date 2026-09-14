/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth.renewal

import android.content.Context
import io.github.b_vitamins.slipbox.security.CredentialVault
import io.github.b_vitamins.slipbox.security.PrivateStoragePolicy
import io.github.b_vitamins.slipbox.security.SlipboxVault
import io.github.b_vitamins.slipbox.security.VaultNamespace
import io.github.b_vitamins.slipbox.security.VaultOutcome
import io.github.b_vitamins.slipbox.security.VaultScope

internal interface RenewalStorage {

    fun open(scope: VaultScope): VaultOutcome<CredentialVault>

    fun policy(): VaultOutcome<PrivateStoragePolicy>
}

internal class SlipboxRenewalStorage(
    context: Context,
    private val namespace: VaultNamespace = VaultNamespace.Application,
) : RenewalStorage {

    private val context = context.applicationContext

    override fun open(scope: VaultScope): VaultOutcome<CredentialVault> =
        SlipboxVault.open(context, namespace, scope)

    override fun policy(): VaultOutcome<PrivateStoragePolicy> =
        SlipboxVault.policy(context, namespace)
}
