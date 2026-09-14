/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.github

import io.github.b_vitamins.slipbox.auth.GithubApp
import io.github.b_vitamins.slipbox.auth.RepositorySelection

class GithubInstallation internal constructor(
    val id: String,
    val accountId: String,
    val accountLogin: String,
    val selection: RepositorySelection,
) {

    override fun toString(): String = "GithubInstallation($id, $selection)"
}

class GithubRepository internal constructor(
    val id: String,
    val installationId: String,
    val ownerAccountId: String,
    val owner: String,
    val name: String,
    val isPrivate: Boolean,
    val defaultBranch: String?,
) {

    /** Derived from confirmed owner/name rather than a provider-supplied URL. */
    val remoteUrl: String = "https://${GithubApp.PROVIDER_AUTHORITY}/$owner/$name.git"

    override fun toString(): String = "GithubRepository($id)"
}

class GithubBranch internal constructor(
    val name: String,
    val commitSha: String,
    val isProtected: Boolean,
) {

    override fun toString(): String = "GithubBranch($name)"
}

enum class GithubEntryKind(internal val description: String) {
    Folder("folder"),
    File("file"),
    Symlink("symlink"),
    Submodule("submodule"),
}

class GithubFolderEntry internal constructor(
    val name: String,
    val path: String,
    val kind: GithubEntryKind,
) {

    override fun toString(): String = "GithubFolderEntry($path, $kind)"
}

enum class GithubCompleteness(internal val description: String) {

    Complete("complete"),

    ProviderTruncated("provider truncated"),

    LocallyCapped("locally capped"),
}

/** A capped or truncated listing is never complete. */
class GithubListing<out T> internal constructor(
    val entries: List<T>,
    val completeness: GithubCompleteness,
) {

    val isComplete: Boolean
        get() = completeness == GithubCompleteness.Complete

    override fun toString(): String = "GithubListing(${entries.size}, $completeness)"
}

class GithubSelectionRequest(
    val accountId: String,
    val installationId: String,
    val repositoryId: String,
    val branch: String,
    val folder: String,
) {

    override fun toString(): String = "GithubSelectionRequest($accountId, $repositoryId)"
}

/** Revalidated selection; the caller supplies its local source identity. */
class ConfirmedGithubSelection internal constructor(
    val accountId: String,
    val ownerAccountId: String,
    val installationId: String,
    val repositoryId: String,
    val owner: String,
    val name: String,
    val remoteUrl: String,
    val branch: String,
    val notesFolder: String,
) {

    override fun toString(): String = "ConfirmedGithubSelection($repositoryId, $branch)"
}
