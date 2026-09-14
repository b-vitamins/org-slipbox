/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth.manual

import android.os.Bundle
import io.github.b_vitamins.slipbox.auth.GithubAuthorization
import io.github.b_vitamins.slipbox.auth.RepositorySelection
import io.github.b_vitamins.slipbox.ui.Record
import java.io.File

internal enum class LiveStage(val word: String) {

    Grant(GRANT_STAGE),

    Cancel(CANCEL_STAGE),
}

internal sealed interface LiveRequest {

    class Given(
        val stage: LiveStage,
        val results: File,
        val expectedAccountId: String?,
        val consentMillis: Long,
    ) : LiveRequest

    class Refused(val reason: String) : LiveRequest
}

/** Invalid arguments are reported by name, not value. */
internal fun liveRequestOf(arguments: Bundle): LiveRequest {
    val stage =
        LiveStage.entries.firstOrNull { it.word == arguments.getString(STAGE_ARGUMENT) }
            ?: return LiveRequest.Refused(
                "-e $STAGE_ARGUMENT must be $GRANT_STAGE or $CANCEL_STAGE",
            )
    val given =
        arguments.getString(RESULTS_ARGUMENT)?.takeIf { it.startsWith('/') }
            ?: return LiveRequest.Refused("-e $RESULTS_ARGUMENT must be an absolute path")
    val results = File(given)
    if (!results.isDirectory && !results.mkdirs()) {
        return LiveRequest.Refused("$RESULTS_ARGUMENT names no directory this run can make")
    }
    if (!results.canWrite()) {
        return LiveRequest.Refused("$RESULTS_ARGUMENT names a directory this run cannot write")
    }
    var expected: String? = null
    if (stage == LiveStage.Grant) {
        val account = arguments.getString(ACCOUNT_ARGUMENT)
        expected =
            account?.takeIf { it.isNotEmpty() && it.all(Char::isDigit) }
                ?: return LiveRequest.Refused(
                    "-e $ACCOUNT_ARGUMENT must be GitHub's numeric account identifier",
                )
    }
    val seconds =
        arguments.getString(CONSENT_ARGUMENT)?.toLongOrNull() ?: DEFAULT_CONSENT_SECONDS
    if (seconds < MIN_CONSENT_SECONDS || seconds > MAX_CONSENT_SECONDS) {
        return LiveRequest.Refused(
            "-e $CONSENT_ARGUMENT must be $MIN_CONSENT_SECONDS-$MAX_CONSENT_SECONDS seconds",
        )
    }
    return LiveRequest.Given(stage, results, expected, seconds * MILLIS_PER_SECOND)
}

internal class LiveGate(authorization: GithubAuthorization, expectedAccountId: String) {

    val identityMatch: Boolean = authorization.account.id == expectedAccountId

    val selectedInstallation: Boolean =
        authorization.access.isInstalled &&
            authorization.access.selection == RepositorySelection.Selected
}

/** Outcome-only report; codes, credentials and identifying inputs are excluded. */
internal class LiveJourneyReport(private val stage: LiveStage) {

    var configured: Boolean = false

    var codeShown: Boolean = false

    var authorized: Boolean = false

    var identityMatch: Boolean = false

    var selectedInstallation: Boolean = false

    var vaultHandoff: Boolean = false

    var vaultReadback: Boolean = false

    var cancellation: Boolean = false

    var ownedCleanup: Boolean = false

    /** Argument refusal or exception class; never include supplied values or messages. */
    var refusal: String? = null

    val passed: Boolean
        get() =
            refusal == null &&
                configured &&
                codeShown &&
                cancellation &&
                ownedCleanup &&
                (stage == LiveStage.Cancel || consented)

    private val consented: Boolean
        get() = authorized && identityMatch && selectedInstallation && vaultHandoff && vaultReadback

    fun summary(): String = "${if (passed) "PASS" else "FAIL"} live-auth ${stage.word}"

    fun json(): String =
        Record()
            .text("stage", stage.word)
            .flag("configured", configured)
            .flag("codeShown", codeShown)
            .flag("authorized", authorized)
            .flag("identityMatch", identityMatch)
            .flag("selectedInstallation", selectedInstallation)
            .flag("vaultHandoff", vaultHandoff)
            .flag("vaultReadback", vaultReadback)
            .flag("cancellation", cancellation)
            .flag("ownedCleanup", ownedCleanup)
            .text("refusal", refusal ?: "none")
            .flag("passed", passed)
            .json()
}

internal const val STAGE_ARGUMENT = "stage"

internal const val RESULTS_ARGUMENT = "resultDir"

internal const val ACCOUNT_ARGUMENT = "expectedAccount"

internal const val CONSENT_ARGUMENT = "consentSeconds"

internal const val GRANT_STAGE = "grant"

internal const val CANCEL_STAGE = "cancel"

internal const val DEFAULT_CONSENT_SECONDS = 300L

internal const val MIN_CONSENT_SECONDS = 30L

internal const val MAX_CONSENT_SECONDS = 900L

private const val MILLIS_PER_SECOND = 1000L
