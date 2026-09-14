/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth

import io.github.b_vitamins.slipbox.security.StoredCredential

/** GitHub-confirmed account; stable numeric ID scopes credentials, login is display-only. */
class VerifiedAccount internal constructor(val id: String, val login: String) {

    override fun equals(other: Any?): Boolean =
        other is VerifiedAccount && other.id == id && other.login == login

    override fun hashCode(): Int = 31 * id.hashCode() + login.hashCode()

    override fun toString(): String = "VerifiedAccount($id)"
}

enum class RepositorySelection {

    None,


    Selected,


    All,
}

/** Installation access is distinct from account authorization. */
class InstallationAccess internal constructor(
    val installations: Int,
    val selection: RepositorySelection,
) {


    val isInstalled: Boolean = installations > 0

    override fun toString(): String = "InstallationAccess($installations, $selection)"
}

/** Verified account, installation access and credential for subsequent repository operations. */
class GithubAuthorization internal constructor(
    val account: VerifiedAccount,
    val access: InstallationAccess,
    val credential: StoredCredential,
) {

    override fun toString(): String = "GithubAuthorization($account, $access, $credential)"
}
