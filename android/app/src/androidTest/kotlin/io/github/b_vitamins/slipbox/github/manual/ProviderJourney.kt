/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.github.manual

import android.os.Bundle
import io.github.b_vitamins.slipbox.github.GithubNames
import io.github.b_vitamins.slipbox.ui.Record
import java.io.File

internal sealed interface ProviderRequest {

    class Given(
        val results: File,
        val expectedAccountId: String,
        val owner: String,
        val repository: String,
        val branch: String,
        val folder: String,
        val consentMillis: Long,
    ) : ProviderRequest

    class Refused(val reason: String) : ProviderRequest
}

/** Refusals name arguments without disclosing their values. */
internal fun providerRequestOf(arguments: Bundle): ProviderRequest {
    val given =
        arguments.getString(RESULTS_ARGUMENT)?.takeIf { it.startsWith('/') }
            ?: return ProviderRequest.Refused("-e $RESULTS_ARGUMENT must be an absolute path")
    val results = File(given)
    if (!results.isDirectory && !results.mkdirs()) {
        return ProviderRequest.Refused("$RESULTS_ARGUMENT names no directory this run can make")
    }
    if (!results.canWrite()) {
        return ProviderRequest.Refused("$RESULTS_ARGUMENT names a directory this run cannot write")
    }
    val account =
        arguments.getString(ACCOUNT_ARGUMENT)?.takeIf { it.isNotEmpty() && it.all(Char::isDigit) }
            ?: return ProviderRequest.Refused(
                "-e $ACCOUNT_ARGUMENT must be GitHub's numeric account identifier",
            )
    val named = (arguments.getString(REPOSITORY_ARGUMENT) ?: "").split('/')
    if (named.size != 2) {
        return ProviderRequest.Refused("-e $REPOSITORY_ARGUMENT must be owner/repository")
    }
    val owner =
        GithubNames.name(named[0])
            ?: return ProviderRequest.Refused("$REPOSITORY_ARGUMENT names no usable owner")
    val repository =
        GithubNames.name(named[1])
            ?: return ProviderRequest.Refused("$REPOSITORY_ARGUMENT names no usable repository")
    val branch =
        arguments.getString(BRANCH_ARGUMENT)?.let(GithubNames::branch)
            ?: return ProviderRequest.Refused("-e $BRANCH_ARGUMENT must be a usable branch")
    val folder =
        GithubNames.folder(arguments.getString(FOLDER_ARGUMENT) ?: "")
            ?: return ProviderRequest.Refused("$FOLDER_ARGUMENT must be a usable folder or absent")
    val seconds =
        arguments.getString(CONSENT_ARGUMENT)?.toLongOrNull() ?: DEFAULT_CONSENT_SECONDS
    if (seconds < MIN_CONSENT_SECONDS || seconds > MAX_CONSENT_SECONDS) {
        return ProviderRequest.Refused(
            "-e $CONSENT_ARGUMENT must be $MIN_CONSENT_SECONDS-$MAX_CONSENT_SECONDS seconds",
        )
    }
    return ProviderRequest.Given(
        results = results,
        expectedAccountId = account,
        owner = owner,
        repository = repository,
        branch = branch,
        folder = folder,
        consentMillis = seconds * MILLIS_PER_SECOND,
    )
}

/** Outcome-only report; excludes identifiers, paths and credentials. */
internal class ProviderJourneyReport {

    var configured: Boolean = false

    var codeShown: Boolean = false

    var authorized: Boolean = false

    var identityMatch: Boolean = false

    var installationsListed: Boolean = false

    var repositoryDiscoverable: Boolean = false

    var branchesListed: Boolean = false

    var branchResolved: Boolean = false

    var folderListed: Boolean = false

    var selectionConfirmed: Boolean = false

    var foreignAccountRefused: Boolean = false

    var unauthorizedRepositoryRefused: Boolean = false

    var ownedCleanup: Boolean = false

    var refusal: String? = null

    val passed: Boolean
        get() =
            refusal == null &&
                configured &&
                codeShown &&
                authorized &&
                identityMatch &&
                discovered &&
                selectionConfirmed &&
                foreignAccountRefused &&
                unauthorizedRepositoryRefused &&
                ownedCleanup

    private val discovered: Boolean
        get() =
            installationsListed &&
                repositoryDiscoverable &&
                branchesListed &&
                branchResolved &&
                folderListed

    fun summary(): String = "${if (passed) "PASS" else "FAIL"} live-github discover"

    fun json(): String =
        Record()
            .flag("configured", configured)
            .flag("codeShown", codeShown)
            .flag("authorized", authorized)
            .flag("identityMatch", identityMatch)
            .flag("installationsListed", installationsListed)
            .flag("repositoryDiscoverable", repositoryDiscoverable)
            .flag("branchesListed", branchesListed)
            .flag("branchResolved", branchResolved)
            .flag("folderListed", folderListed)
            .flag("selectionConfirmed", selectionConfirmed)
            .flag("foreignAccountRefused", foreignAccountRefused)
            .flag("unauthorizedRepositoryRefused", unauthorizedRepositoryRefused)
            .flag("ownedCleanup", ownedCleanup)
            .text("refusal", refusal ?: "none")
            .flag("passed", passed)
            .json()
}

internal const val RESULTS_ARGUMENT = "resultDir"

internal const val ACCOUNT_ARGUMENT = "expectedAccount"

internal const val REPOSITORY_ARGUMENT = "repository"

internal const val BRANCH_ARGUMENT = "branch"

internal const val FOLDER_ARGUMENT = "folder"

internal const val CONSENT_ARGUMENT = "consentSeconds"

internal const val DEFAULT_CONSENT_SECONDS = 300L

internal const val MIN_CONSENT_SECONDS = 30L

internal const val MAX_CONSENT_SECONDS = 900L

internal const val ABSENT_IDENTIFIER = "0"

private const val MILLIS_PER_SECOND = 1000L
