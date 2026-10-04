/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.git.manual

import io.github.b_vitamins.slipbox.ui.Record

internal object LiveGitContract {

    const val RESULT_ARGUMENT = "resultDir"
    const val ONLINE_RESULT_FILE = "live-git.json"
    const val OFFLINE_RESULT_FILE = "live-git-offline.json"
    const val PUBLIC_SOURCE_ID = "11111111111111111111111111111111"
    const val PRIVATE_SOURCE_ID = "22222222222222222222222222222222"
}

/** Sanitized evidence from one explicitly invoked private-repository journey. */
internal class GitJourneyReport {

    var inputAccepted: Boolean = false

    var tokenRenewed: Boolean = false

    var renewalCommitted: Boolean = false

    var publicClone: Boolean = false

    var publicMaterialized: Boolean = false

    var privateClone: Boolean = false

    var authenticatedFetch: Boolean = false

    var publicGenerationReady: Boolean = false

    var privateGenerationReady: Boolean = false

    var privateReadsReady: Boolean = false

    var sourceSwitching: Boolean = false

    var readingStateStored: Boolean = false

    var ownedCleanup: Boolean = false

    var refusal: String? = null

    val passed: Boolean
        get() =
            refusal == null &&
                inputAccepted &&
                tokenRenewed &&
                renewalCommitted &&
                publicClone &&
                publicMaterialized &&
                privateClone &&
                authenticatedFetch &&
                publicGenerationReady &&
                privateGenerationReady &&
                privateReadsReady &&
                sourceSwitching &&
                readingStateStored &&
                ownedCleanup

    fun summary(): String =
        "${if (passed) "PASS" else "FAIL"} live-git source-to-reading"

    fun json(): String =
        Record()
            .flag("inputAccepted", inputAccepted)
            .flag("tokenRenewed", tokenRenewed)
            .flag("renewalCommitted", renewalCommitted)
            .flag("publicClone", publicClone)
            .flag("publicMaterialized", publicMaterialized)
            .flag("privateClone", privateClone)
            .flag("authenticatedFetch", authenticatedFetch)
            .flag("publicGenerationReady", publicGenerationReady)
            .flag("privateGenerationReady", privateGenerationReady)
            .flag("privateReadsReady", privateReadsReady)
            .flag("sourceSwitching", sourceSwitching)
            .flag("readingStateStored", readingStateStored)
            .flag("ownedCleanup", ownedCleanup)
            .text("refusal", refusal ?: "none")
            .flag("passed", passed)
            .json()
}

/** Sanitized evidence from a process restarted with airplane mode enabled. */
internal class OfflineGitJourneyReport {

    var airplaneMode: Boolean = false

    var catalogRestored: Boolean = false

    var generationRestored: Boolean = false

    var notesPaged: Boolean = false

    var glossaryPaged: Boolean = false

    var relationsRead: Boolean = false

    var searchRead: Boolean = false

    var trailRestored: Boolean = false

    var returnsRestored: Boolean = false

    var refusal: String? = null

    val passed: Boolean
        get() =
            refusal == null &&
                airplaneMode &&
                catalogRestored &&
                generationRestored &&
                notesPaged &&
                glossaryPaged &&
                relationsRead &&
                searchRead &&
                trailRestored &&
                returnsRestored

    fun summary(): String =
        "${if (passed) "PASS" else "FAIL"} live-git airplane-mode-restart"

    fun json(): String =
        Record()
            .flag("airplaneMode", airplaneMode)
            .flag("catalogRestored", catalogRestored)
            .flag("generationRestored", generationRestored)
            .flag("notesPaged", notesPaged)
            .flag("glossaryPaged", glossaryPaged)
            .flag("relationsRead", relationsRead)
            .flag("searchRead", searchRead)
            .flag("trailRestored", trailRestored)
            .flag("returnsRestored", returnsRestored)
            .text("refusal", refusal ?: "none")
            .flag("passed", passed)
            .json()
}
