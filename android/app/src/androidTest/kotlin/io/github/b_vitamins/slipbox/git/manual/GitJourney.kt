/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.git.manual

import io.github.b_vitamins.slipbox.ui.Record

/** Sanitized evidence from one explicitly invoked private-repository journey. */
internal class GitJourneyReport {

    var inputAccepted: Boolean = false

    var tokenRenewed: Boolean = false

    var renewalCommitted: Boolean = false

    var publicClone: Boolean = false

    var publicMaterialized: Boolean = false

    var privateClone: Boolean = false

    var authenticatedFetch: Boolean = false

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
                ownedCleanup

    fun summary(): String =
        "${if (passed) "PASS" else "FAIL"} live-git clone/materialize/fetch"

    fun json(): String =
        Record()
            .flag("inputAccepted", inputAccepted)
            .flag("tokenRenewed", tokenRenewed)
            .flag("renewalCommitted", renewalCommitted)
            .flag("publicClone", publicClone)
            .flag("publicMaterialized", publicMaterialized)
            .flag("privateClone", privateClone)
            .flag("authenticatedFetch", authenticatedFetch)
            .flag("ownedCleanup", ownedCleanup)
            .text("refusal", refusal ?: "none")
            .flag("passed", passed)
            .json()
}
