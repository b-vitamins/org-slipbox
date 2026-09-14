/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

package io.github.b_vitamins.slipbox.auth.renewal

import io.github.b_vitamins.slipbox.security.StoredCredential
import io.github.b_vitamins.slipbox.security.VaultFailure

sealed interface RenewalOutcome {

    sealed interface Usable : RenewalOutcome {

        val credential: StoredCredential
    }

    data class Current(override val credential: StoredCredential) : Usable

    data class Renewed(override val credential: StoredCredential) : Usable

    data class Reauthorize(val reason: ReauthorizationReason) : RenewalOutcome

    /** Rotation succeeded but its local commit failed or is uncertain. */
    data class Uncommitted(val failure: VaultFailure) : RenewalOutcome

    /** No usable reply; local storage is unchanged. */
    data class Unavailable(val fault: RenewalFault) : RenewalOutcome

    object Withdrawn : RenewalOutcome
}

enum class ReauthorizationReason {

    NoStoredCredential,

    NoRefreshAuthorization,

    StoredCredentialUnusable,

    RefreshRejected,

    AccessRevoked,

    RotationLost,
}

sealed interface RenewalRemoval {

    object Removed : RenewalRemoval

    /** Removal was partial; retry resumes from the remaining data. */
    data class Incomplete(val failure: VaultFailure) : RenewalRemoval

    object Withdrawn : RenewalRemoval
}
