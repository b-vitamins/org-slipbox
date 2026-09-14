/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.github

/** Refusals exclude response bodies, header text and exception messages. */
sealed class GithubRefusal {

    abstract val stage: GithubStage

    open val recovery: GithubRecovery? = null

    internal abstract val description: String

    final override fun toString(): String = "GithubRefusal(${stage.description}, $description)"

    data class AuthorizationExpired(override val stage: GithubStage) : GithubRefusal() {

        override val recovery: GithubRecovery = GithubRecovery(GithubRecoveryAction.Renew)

        override val description: String = "authorization expired"
    }

    data class AccessRestricted(
        override val stage: GithubStage,
        val restriction: AccessRestriction,
        override val recovery: GithubRecovery?,
    ) : GithubRefusal() {

        override val description: String = restriction.description
    }

    data class SignOnRequired(
        override val stage: GithubStage,
        override val recovery: GithubRecovery?,
    ) : GithubRefusal() {

        override val description: String = "single sign-on required"
    }

    data class RateLimited(
        override val stage: GithubStage,
        val retryAfterSeconds: Long?,
        val resetEpochSeconds: Long?,
    ) : GithubRefusal() {

        override val recovery: GithubRecovery = GithubRecovery(GithubRecoveryAction.Wait)

        override val description: String = "rate limited"
    }

    data class ResourceHidden(override val stage: GithubStage) : GithubRefusal() {

        override val recovery: GithubRecovery = GithubRecovery(GithubRecoveryAction.Reselect)

        override val description: String = "absent or invisible"
    }

    data class SelectionStale(
        override val stage: GithubStage,
        val part: StaleSelection,
    ) : GithubRefusal() {

        override val recovery: GithubRecovery = GithubRecovery(GithubRecoveryAction.Reselect)

        override val description: String = "stale ${part.description}"
    }

    data class SelectionUnusable(
        override val stage: GithubStage,
        val field: SelectionField,
    ) : GithubRefusal() {

        override val recovery: GithubRecovery = GithubRecovery(GithubRecoveryAction.Reselect)

        override val description: String = "unusable ${field.description}"
    }

    data class ListingIncomplete(
        override val stage: GithubStage,
        val completeness: GithubCompleteness,
    ) : GithubRefusal() {

        override val recovery: GithubRecovery = GithubRecovery(GithubRecoveryAction.Refine)

        override val description: String = completeness.description
    }

    /** Files, symlinks and submodules cannot be traversed as folders. */
    data class EntryNotFolder(
        override val stage: GithubStage,
        val kind: GithubEntryKind,
    ) : GithubRefusal() {

        override val recovery: GithubRecovery = GithubRecovery(GithubRecoveryAction.Reselect)

        override val description: String = "not a folder: ${kind.description}"
    }

    data class Redirected(override val stage: GithubStage) : GithubRefusal() {

        override val description: String = "redirected"
    }

    data class TransportFailed(
        override val stage: GithubStage,
        val origin: String,
    ) : GithubRefusal() {

        override val description: String = "transport $origin"
    }

    data class UnexpectedStatus(
        override val stage: GithubStage,
        val status: Int,
    ) : GithubRefusal() {

        override val description: String = "status $status"
    }

    data class MalformedAnswer(
        override val stage: GithubStage,
        val defect: GithubDefect,
    ) : GithubRefusal() {

        override val description: String = defect.description
    }

    data class Cancelled(override val stage: GithubStage) : GithubRefusal() {

        override val description: String = "cancelled"
    }
}

enum class GithubStage(internal val description: String) {
    Installations("installations"),
    Repositories("repositories"),
    Branches("branches"),
    Branch("branch"),
    Folder("folder"),
    Selection("selection"),
    Traversal("traversal"),
}

enum class AccessRestriction(internal val description: String) {

    InstallationMissing("no installation"),

    RepositoryUnselected("repository not selected"),

    PermissionMissing("permission missing"),
}

enum class StaleSelection(internal val description: String) {
    Account("account"),
    Installation("installation"),
}

enum class SelectionField(internal val description: String) {
    Branch("branch"),
    Folder("folder"),
}

enum class GithubDefect(internal val description: String) {
    NotText("not text"),
    NotJson("not an object"),
    FieldMissing("field missing"),
    FieldUnusable("field unusable"),
    TooLarge("answer too large"),

    ForeignDestination("foreign destination"),

    AddressRefused("address refused"),
}
