/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.github

import io.github.b_vitamins.slipbox.auth.GithubApp
import io.github.b_vitamins.slipbox.auth.GithubAuthorization
import io.github.b_vitamins.slipbox.auth.RepositorySelection
import kotlinx.serialization.DeserializationStrategy
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.Json

sealed interface GithubOutcome<out T> {

    class Read<out T> internal constructor(val value: T) : GithubOutcome<T>

    class Refused internal constructor(val refusal: GithubRefusal) : GithubOutcome<Nothing>
}

/** Bounded, blocking discovery within the user/App permission intersection. */
class GithubProvider internal constructor(
    private val authorization: GithubAuthorization,
    private val app: GithubApp?,
    private val transport: GithubApiTransport,
    private val live: () -> Boolean,
) {

    fun installations(): GithubOutcome<GithubListing<GithubInstallation>> {
        val stage = GithubStage.Installations
        val collected =
            collect(stage, INSTALLATIONS, InstallationsPage.serializer(), ::installationsOf)
        val listing =
            when (collected) {
                is GithubOutcome.Refused -> return collected
                is GithubOutcome.Read -> collected.value
            }
        if (listing.entries.isEmpty() && listing.isComplete) {
            return refused(restriction(stage, AccessRestriction.InstallationMissing))
        }
        return read(listing)
    }

    fun repositories(
        installation: GithubInstallation,
    ): GithubOutcome<GithubListing<GithubRepository>> {
        val stage = GithubStage.Repositories
        val id =
            GithubNames.name(installation.id)
                ?: return refused(malformed(stage, GithubDefect.FieldUnusable))
        val path = "$INSTALLATIONS/${GithubEncoding.segment(id)}/repositories"
        val collected =
            collect(stage, path, RepositoriesPage.serializer()) { repositoriesOf(installation, it) }
        val listing =
            when (collected) {
                is GithubOutcome.Refused -> return collected
                is GithubOutcome.Read -> collected.value
            }
        // An installation restricted to selected repositories that shows none is a refusal,
        // not an account without repositories.
        if (listing.entries.isEmpty() &&
            listing.isComplete &&
            installation.selection == RepositorySelection.Selected
        ) {
            return refused(restriction(stage, AccessRestriction.RepositoryUnselected))
        }
        return read(listing)
    }

    fun branches(repository: GithubRepository): GithubOutcome<GithubListing<GithubBranch>> {
        val stage = GithubStage.Branches
        val base =
            repositoryPath(repository)
                ?: return refused(malformed(stage, GithubDefect.FieldUnusable))
        val serializer = ListSerializer(BranchEntry.serializer())
        return collect(stage, "$base/branches", serializer, ::branchesOf)
    }

    /** Pins subsequent folder reads to a verified branch commit. */
    fun branch(repository: GithubRepository, name: String): GithubOutcome<GithubBranch> {
        val stage = GithubStage.Branch
        val canonical =
            GithubNames.branch(name)
                ?: return refused(GithubRefusal.SelectionUnusable(stage, SelectionField.Branch))
        val base =
            repositoryPath(repository)
                ?: return refused(malformed(stage, GithubDefect.FieldUnusable))
        val address =
            GithubAddress.of("$base/branches/${GithubEncoding.path(canonical)}")
                ?: return refused(malformed(stage, GithubDefect.AddressRefused))
        val answer =
            when (val reply = answered(address, stage)) {
                is Answer.Refused -> return refused(reply.refusal)
                is Answer.Body -> reply
            }
        val entry =
            decode(answer.text, BranchEntry.serializer())
                ?: return refused(malformed(stage, GithubDefect.NotJson))
        val branch =
            branchOf(entry)?.takeIf { it.name == canonical }
                ?: return refused(malformed(stage, GithubDefect.FieldUnusable))
        return read(branch)
    }

    /** Reads only trees along the requested path and reports truncation. */
    fun folder(
        repository: GithubRepository,
        branch: GithubBranch,
        folder: String,
    ): GithubOutcome<GithubListing<GithubFolderEntry>> {
        val stage = GithubStage.Folder
        val canonical =
            GithubNames.folder(folder)
                ?: return refused(GithubRefusal.SelectionUnusable(stage, SelectionField.Folder))
        val located = locate(repository, branch, canonical)
        val tree =
            when (located) {
                is GithubOutcome.Refused -> return located
                is GithubOutcome.Read -> located.value
            }
        val entries = ArrayList<GithubFolderEntry>()
        for (entry in tree.tree.orEmpty()) {
            val name =
                entry.path?.takeIf { isEntryName(it) }
                    ?: return refused(malformed(stage, GithubDefect.FieldUnusable))
            val kind = kindOf(entry) ?: return refused(malformed(stage, GithubDefect.FieldUnusable))
            entries.add(
                GithubFolderEntry(name, if (canonical.isEmpty()) name else "$canonical/$name", kind),
            )
            if (entries.size >= MAX_ENTRIES) {
                return read(GithubListing(entries.toList(), GithubCompleteness.LocallyCapped))
            }
        }
        val completeness =
            if (tree.truncated) {
                GithubCompleteness.ProviderTruncated
            } else {
                GithubCompleteness.Complete
            }
        return read(GithubListing(entries.toList(), completeness))
    }

    /** Revalidates account/repository IDs and branch/folder access. */
    fun confirm(request: GithubSelectionRequest): GithubOutcome<ConfirmedGithubSelection> {
        val stage = GithubStage.Selection
        if (request.accountId != authorization.account.id) {
            return refused(GithubRefusal.SelectionStale(stage, StaleSelection.Account))
        }
        val installations =
            when (val step = installations()) {
                is GithubOutcome.Refused -> return step
                is GithubOutcome.Read -> step.value
            }
        val installation =
            installations.entries.firstOrNull { it.id == request.installationId }
                ?: return refused(
                    if (installations.isComplete) {
                        GithubRefusal.SelectionStale(stage, StaleSelection.Installation)
                    } else {
                        GithubRefusal.ListingIncomplete(stage, installations.completeness)
                    },
                )
        val repositories =
            when (val step = repositories(installation)) {
                is GithubOutcome.Refused -> return step
                is GithubOutcome.Read -> step.value
            }
        val repository =
            repositories.entries.firstOrNull { it.id == request.repositoryId }
                ?: return refused(
                    if (repositories.isComplete) {
                        restriction(stage, AccessRestriction.RepositoryUnselected)
                    } else {
                        GithubRefusal.ListingIncomplete(stage, repositories.completeness)
                    },
                )
        val branch =
            when (val step = branch(repository, request.branch)) {
                is GithubOutcome.Refused -> return step
                is GithubOutcome.Read -> step.value
            }
        val notes =
            GithubNames.folder(request.folder)
                ?: return refused(GithubRefusal.SelectionUnusable(stage, SelectionField.Folder))
        if (notes.isNotEmpty()) {
            when (val step = folder(repository, branch, notes)) {
                is GithubOutcome.Refused -> return step
                is GithubOutcome.Read -> Unit
            }
        }
        return read(
            ConfirmedGithubSelection(
                accountId = authorization.account.id,
                ownerAccountId = repository.ownerAccountId,
                installationId = installation.id,
                repositoryId = repository.id,
                owner = repository.owner,
                name = repository.name,
                remoteUrl = repository.remoteUrl,
                branch = branch.name,
                notesFolder = notes,
            ),
        )
    }

    private fun <R, E> collect(
        stage: GithubStage,
        path: String,
        serializer: DeserializationStrategy<R>,
        entries: (R) -> List<E>?,
    ): GithubOutcome<GithubListing<E>> {
        var address =
            GithubAddress.paged(path)
                ?: return refused(malformed(stage, GithubDefect.AddressRefused))
        val collected = ArrayList<E>()
        var pages = 0
        while (true) {
            if (!live()) {
                return refused(GithubRefusal.Cancelled(stage))
            }
            val answer =
                when (val reply = answered(address, stage)) {
                    is Answer.Refused -> return refused(reply.refusal)
                    is Answer.Body -> reply
                }
            val decoded =
                decode(answer.text, serializer)
                    ?: return refused(malformed(stage, GithubDefect.NotJson))
            val values =
                entries(decoded) ?: return refused(malformed(stage, GithubDefect.FieldUnusable))
            collected.addAll(values)
            pages++
            val destination =
                GithubAddress.nextTarget(answer.headers.link)
                    ?: return read(GithubListing(collected.toList(), GithubCompleteness.Complete))
            if (collected.size >= MAX_ENTRIES || pages >= MAX_PAGES) {
                return read(GithubListing(collected.toList(), GithubCompleteness.LocallyCapped))
            }
            address =
                address.continuation(destination)
                    ?: return refused(malformed(stage, GithubDefect.ForeignDestination))
        }
    }

    private fun locate(
        repository: GithubRepository,
        branch: GithubBranch,
        canonical: String,
    ): GithubOutcome<TreeReply> {
        val stage = GithubStage.Folder
        var sha =
            GithubNames.commit(branch.commitSha)
                ?: return refused(malformed(stage, GithubDefect.FieldUnusable))
        if (canonical.isEmpty()) {
            return readTree(repository, sha)
        }
        for (segment in canonical.split('/')) {
            val level =
                when (val step = readTree(repository, sha)) {
                    is GithubOutcome.Refused -> return step
                    is GithubOutcome.Read -> step.value
                }
            val entry =
                level.tree.orEmpty().firstOrNull { it.path == segment }
                    ?: return refused(hidden(stage, level.truncated))
            val kind = kindOf(entry) ?: return refused(malformed(stage, GithubDefect.FieldUnusable))
            if (kind != GithubEntryKind.Folder) {
                return refused(GithubRefusal.EntryNotFolder(stage, kind))
            }
            sha =
                entry.sha?.let(GithubNames::commit)
                    ?: return refused(malformed(stage, GithubDefect.FieldMissing))
        }
        return readTree(repository, sha)
    }

    private fun readTree(repository: GithubRepository, sha: String): GithubOutcome<TreeReply> {
        val stage = GithubStage.Folder
        if (!live()) {
            return refused(GithubRefusal.Cancelled(stage))
        }
        val base =
            repositoryPath(repository)
                ?: return refused(malformed(stage, GithubDefect.FieldUnusable))
        val address =
            GithubAddress.of("$base/git/trees/${GithubEncoding.segment(sha)}")
                ?: return refused(malformed(stage, GithubDefect.AddressRefused))
        val answer =
            when (val reply = answered(address, stage)) {
                is Answer.Refused -> return refused(reply.refusal)
                is Answer.Body -> reply
            }
        val tree =
            decode(answer.text, TreeReply.serializer())
                ?: return refused(malformed(stage, GithubDefect.NotJson))
        if (tree.tree == null) {
            return refused(malformed(stage, GithubDefect.FieldUnusable))
        }
        return read(tree)
    }

    private fun answered(address: GithubAddress, stage: GithubStage): Answer {
        val request = GithubApiRequest(address, authorization.credential.accessToken)
        return when (val reply = transport.fetch(request)) {
            is GithubApiReply.Failed ->
                Answer.Refused(GithubRefusal.TransportFailed(stage, reply.origin))
            is GithubApiReply.Answered -> classified(reply, stage)
        }
    }

    private fun classified(reply: GithubApiReply.Answered, stage: GithubStage): Answer =
        when {
            reply.status == OK ->
                when {
                    reply.bodyExceeded -> Answer.Refused(malformed(stage, GithubDefect.TooLarge))
                    reply.body.isEmpty() -> Answer.Refused(malformed(stage, GithubDefect.NotText))
                    else -> Answer.Body(reply.body, reply.headers)
                }
            reply.status in REDIRECTION -> Answer.Refused(GithubRefusal.Redirected(stage))
            reply.status == UNAUTHORIZED ->
                Answer.Refused(GithubRefusal.AuthorizationExpired(stage))
            reply.status == FORBIDDEN || reply.status == TOO_MANY_REQUESTS ->
                Answer.Refused(restricted(reply, stage))
            reply.status == NOT_FOUND -> Answer.Refused(GithubRefusal.ResourceHidden(stage))
            else -> Answer.Refused(GithubRefusal.UnexpectedStatus(stage, reply.status))
        }

    private fun restricted(reply: GithubApiReply.Answered, stage: GithubStage): GithubRefusal {
        val headers = reply.headers
        if (headers.singleSignOn != null) {
            return GithubRefusal.SignOnRequired(
                stage,
                GithubRecovery.signOn(headers.singleSignOn, app?.installationUrl),
            )
        }
        val limited =
            reply.status == TOO_MANY_REQUESTS ||
                headers.retryAfterSeconds != null ||
                headers.rateLimitRemaining == 0L
        if (limited) {
            return GithubRefusal.RateLimited(
                stage,
                headers.retryAfterSeconds,
                headers.rateLimitResetEpochSeconds,
            )
        }
        return restriction(stage, AccessRestriction.PermissionMissing)
    }

    private fun restriction(stage: GithubStage, restriction: AccessRestriction): GithubRefusal =
        GithubRefusal.AccessRestricted(
            stage,
            restriction,
            GithubRecovery.install(app?.installationUrl),
        )

    private fun repositoryPath(repository: GithubRepository): String? {
        val owner = GithubNames.name(repository.owner) ?: return null
        val name = GithubNames.name(repository.name) ?: return null
        return "/repos/${GithubEncoding.segment(owner)}/${GithubEncoding.segment(name)}"
    }

    private fun installationsOf(page: InstallationsPage): List<GithubInstallation>? {
        val entries = page.installations ?: return null
        return entries.map { entry ->
            val id = entry.id?.takeIf { it > 0 } ?: return null
            val account = entry.account ?: return null
            val accountId = account.id?.takeIf { it > 0 } ?: return null
            val login = account.login?.let(GithubNames::name) ?: return null
            GithubInstallation(
                id.toString(),
                accountId.toString(),
                login,
                selectionOf(entry.repositorySelection),
            )
        }
    }

    private fun repositoriesOf(
        installation: GithubInstallation,
        page: RepositoriesPage,
    ): List<GithubRepository>? {
        val entries = page.repositories ?: return null
        return entries.map { entry ->
            val id = entry.id?.takeIf { it > 0 } ?: return null
            val name = entry.name?.let(GithubNames::name) ?: return null
            val owner = entry.owner ?: return null
            val ownerId = owner.id?.takeIf { it > 0 } ?: return null
            val login = owner.login?.let(GithubNames::name) ?: return null
            GithubRepository(
                id = id.toString(),
                installationId = installation.id,
                ownerAccountId = ownerId.toString(),
                owner = login,
                name = name,
                isPrivate = entry.isPrivate,
                defaultBranch = entry.defaultBranch?.let(GithubNames::branch),
            )
        }
    }

    private fun branchesOf(page: List<BranchEntry>): List<GithubBranch>? =
        page.map { branchOf(it) ?: return null }

    private fun branchOf(entry: BranchEntry): GithubBranch? {
        val name = entry.name?.let(GithubNames::branch) ?: return null
        val sha = entry.commit?.sha?.let(GithubNames::commit) ?: return null
        return GithubBranch(name, sha, entry.isProtected)
    }

    private fun <T> read(value: T): GithubOutcome<T> = GithubOutcome.Read(value)

    private fun refused(refusal: GithubRefusal): GithubOutcome<Nothing> =
        GithubOutcome.Refused(refusal)

    private sealed interface Answer {

        class Body(val text: String, val headers: GithubApiHeaders) : Answer

        class Refused(val refusal: GithubRefusal) : Answer
    }

    companion object {

        internal fun packaged(
            authorization: GithubAuthorization,
            live: () -> Boolean = { true },
        ): GithubProvider =
            GithubProvider(authorization, GithubApp.packaged(), HttpsGithubApiTransport(), live)

        private const val INSTALLATIONS = "/user/installations"

        private const val OK = 200

        private const val UNAUTHORIZED = 401

        private const val FORBIDDEN = 403

        private const val NOT_FOUND = 404

        private const val TOO_MANY_REQUESTS = 429

        private val REDIRECTION = 300..399

        private const val MAX_ENTRIES = 4_000

        private const val MAX_PAGES = 40

        private const val MAX_ENTRY_NAME_CHARS = 255

        private const val TREE = "tree"

        private const val BLOB = "blob"

        private const val SUBMODULE = "commit"

        private const val SYMLINK_MODE = "120000"

        private const val SELECTION_ALL = "all"

        private val json = Json { ignoreUnknownKeys = true }

        // Parser messages can quote credential-bearing responses.
        private fun <T> decode(body: String, serializer: DeserializationStrategy<T>): T? =
            try {
                json.decodeFromString(serializer, body)
            } catch (fault: IllegalArgumentException) {
                null
            }

        private fun malformed(stage: GithubStage, defect: GithubDefect): GithubRefusal =
            GithubRefusal.MalformedAnswer(stage, defect)

        // A truncated level cannot prove a missing name is absent.
        private fun hidden(stage: GithubStage, truncated: Boolean): GithubRefusal =
            if (truncated) {
                GithubRefusal.ListingIncomplete(stage, GithubCompleteness.ProviderTruncated)
            } else {
                GithubRefusal.ResourceHidden(stage)
            }

        // An unfamiliar selection is read as restricted rather than as unrestricted access.
        private fun selectionOf(value: String?): RepositorySelection =
            if (value == SELECTION_ALL) RepositorySelection.All else RepositorySelection.Selected

        private fun kindOf(entry: TreeEntry): GithubEntryKind? =
            when (entry.type) {
                TREE -> GithubEntryKind.Folder
                SUBMODULE -> GithubEntryKind.Submodule
                BLOB ->
                    if (entry.mode == SYMLINK_MODE) {
                        GithubEntryKind.Symlink
                    } else {
                        GithubEntryKind.File
                    }
                else -> null
            }

        private fun isEntryName(value: String): Boolean =
            value.isNotEmpty() &&
                value.length <= MAX_ENTRY_NAME_CHARS &&
                value != "." &&
                value != ".." &&
                value.none { it == '/' || it.code < 0x20 || it.code == 0x7f }
    }
}

@Serializable
private class InstallationsPage(val installations: List<InstallationEntry>? = null)

@Serializable
private class InstallationEntry(
    val id: Long? = null,
    val account: AccountEntry? = null,
    @SerialName("repository_selection") val repositorySelection: String? = null,
)

@Serializable
private class AccountEntry(val id: Long? = null, val login: String? = null)

@Serializable
private class RepositoriesPage(val repositories: List<RepositoryEntry>? = null)

@Serializable
private class RepositoryEntry(
    val id: Long? = null,
    val name: String? = null,
    val owner: AccountEntry? = null,
    @SerialName("private") val isPrivate: Boolean = false,
    @SerialName("default_branch") val defaultBranch: String? = null,
)

@Serializable
private class BranchEntry(
    val name: String? = null,
    val commit: CommitEntry? = null,
    @SerialName("protected") val isProtected: Boolean = false,
)

@Serializable
private class CommitEntry(val sha: String? = null)

@Serializable
private class TreeReply(
    val tree: List<TreeEntry>? = null,
    val truncated: Boolean = false,
)

@Serializable
private class TreeEntry(
    val path: String? = null,
    val mode: String? = null,
    val type: String? = null,
    val sha: String? = null,
)
